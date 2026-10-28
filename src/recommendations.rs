//! Conservative cleanup suggestions from an existing metadata snapshot.
//!
//! This module never reads file contents, opens paths, or deletes anything. Names
//! and dates provide evidence for review, not a guarantee that data is disposable.
use crate::scanner::{Node, Scan};
use serde::Serialize;
use std::{
    cmp::{Ordering, Reverse},
    collections::BinaryHeap,
};

const MIB: u64 = 1024 * 1024;
const DAY: i64 = 86_400;
const MAX_ITEMS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    WindowsCleanup,
    Regenerable,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    StorageSettings,
    RecycleBin,
    Inspect,
}

#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub node: usize,
    pub title: String,
    pub reason: String,
    pub caution: String,
    pub category: Category,
    /// Logical bytes observed in the snapshot, never promised physical savings.
    pub bytes: u64,
    pub action: Action,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub items: Vec<Recommendation>,
    /// Non-overlapping logical bytes for the candidates actually listed.
    pub regenerable_bytes: u64,
    pub review_bytes: u64,
    pub inspected_files: u64,
    pub incomplete: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, Default)]
enum Location {
    Drive,
    Users,
    User,
    AppData,
    Local,
    Windows,
    Temp,
    Recycle,
    OldWindows,
    #[default]
    Other,
}

#[derive(Debug, Clone, Copy, Default)]
struct Context {
    location: Location,
    blocked: bool,
    personal: bool,
    downloads: bool,
    cloud: bool,
    covered: bool,
}

impl Context {
    fn child(self, name: &str, directory: bool) -> Self {
        let mut next = self;
        next.location = if directory {
            match (self.location, name) {
                (Location::Drive, "users") => Location::Users,
                (Location::Users, _) => Location::User,
                (Location::User, "appdata") => Location::AppData,
                (Location::AppData, "local") => Location::Local,
                (Location::Local | Location::Windows, "temp") => Location::Temp,
                (Location::Drive, "windows") => Location::Windows,
                (Location::Drive, "$recycle.bin") => Location::Recycle,
                (Location::Drive, "windows.old") => Location::OldWindows,
                _ => Location::Other,
            }
        } else {
            Location::Other
        };
        if directory {
            next.blocked |= matches!(
                name,
                "windows"
                    | "windows.old"
                    | "program files"
                    | "program files (x86)"
                    | "programdata"
                    | "system volume information"
                    | "$recycle.bin"
                    | ".git"
                    | "node_modules"
                    | "appdata"
            );
            next.downloads |= matches!(name, "downloads" | "transferências" | "transferencias");
            next.personal |= matches!(
                name,
                "downloads"
                    | "transferências"
                    | "transferencias"
                    | "documents"
                    | "documentos"
                    | "desktop"
                    | "ambiente de trabalho"
                    | "pictures"
                    | "imagens"
                    | "videos"
                    | "vídeos"
                    | "music"
                    | "música"
            );
            next.cloud |=
                name.starts_with("onedrive") || name.starts_with("sharepoint") || name == "dropbox";
        }
        next
    }

    fn root(scan: &Scan) -> Self {
        // Parsing the display spelling works on both Windows and test hosts. It
        // is used only to classify metadata, never to construct filesystem paths.
        let path = scan
            .root
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();
        let path = path.strip_prefix("//?/").unwrap_or(&path);
        let mut context = Self::default();
        for (i, name) in path.split('/').filter(|p| !p.is_empty()).enumerate() {
            if i == 0 && name.len() == 2 && name.ends_with(':') {
                context.location = Location::Drive;
            } else {
                context = context.child(name, true);
            }
        }
        context
    }
}

fn has_file(scan: &Scan, index: usize, name: &str) -> bool {
    scan.nodes[index].children.iter().any(|i| {
        !scan.nodes[*i].directory
            && scan.nodes[*i]
                .name
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
    })
}

fn has_directory(scan: &Scan, index: usize, name: &str) -> bool {
    scan.nodes[index].children.iter().any(|i| {
        scan.nodes[*i].directory
            && scan.nodes[*i]
                .name
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
    })
}

fn cargo_output(scan: &Scan, index: usize) -> bool {
    let Some(parent) = scan.nodes[index].parent else {
        return false;
    };
    if !has_file(scan, parent, "Cargo.toml")
        || !has_file(scan, index, ".rustc_info.json")
        || !has_file(scan, index, "CACHEDIR.TAG")
    {
        return false;
    }
    if !has_directory(scan, index, "debug") && !has_directory(scan, index, "release") {
        return false;
    }
    // Unexpected top-level content prevents a regenerable classification. A user
    // can store arbitrary files even inside a conventionally named build folder.
    scan.nodes[index].children.iter().all(|i| {
        let child = &scan.nodes[*i];
        let name = child.name.to_string_lossy().to_ascii_lowercase();
        if child.directory {
            matches!(
                name.as_str(),
                "debug" | "release" | "doc" | "package" | "tmp"
            )
        } else {
            matches!(name.as_str(), ".rustc_info.json" | "cachedir.tag")
        }
    })
}

fn next_output(scan: &Scan, index: usize) -> bool {
    let Some(parent) = scan.nodes[index].parent else {
        return false;
    };
    has_file(scan, parent, "package.json")
        && has_directory(scan, index, "server")
        && has_file(scan, index, "build-manifest.json")
        && (has_file(scan, index, "BUILD_ID") || has_file(scan, index, "app-build-manifest.json"))
        && scan.nodes[index].children.iter().all(|i| {
            let child = &scan.nodes[*i];
            let name = child.name.to_string_lossy().to_ascii_lowercase();
            if child.directory {
                matches!(
                    name.as_str(),
                    "cache"
                        | "server"
                        | "static"
                        | "types"
                        | "diagnostics"
                        | "standalone"
                        | "dev"
                        | "build"
                )
            } else {
                matches!(
                    name.as_str(),
                    "build_id"
                        | "build-manifest.json"
                        | "app-build-manifest.json"
                        | "routes-manifest.json"
                        | "prerender-manifest.json"
                        | "react-loadable-manifest.json"
                        | "required-server-files.json"
                        | "images-manifest.json"
                        | "export-marker.json"
                        | "export-detail.json"
                        | "package.json"
                        | "next-server.js.nft.json"
                        | "next-minimal-server.js.nft.json"
                        | "trace"
                        | "trace-build"
                        | "trace-turbopack"
                )
            }
        })
}

fn generated(scan: &Scan, index: usize, context: Context, name: &str) -> Option<Recommendation> {
    let node = &scan.nodes[index];
    if !node.directory || node.bytes < 32 * MIB {
        return None;
    }
    let (title, reason, caution) = if name == "target" && cargo_output(scan, index) {
        (
            "Saída de compilação Rust",
            "Existe Cargo.toml na pasta superior, marcadores Cargo e perfis de compilação na pasta target.",
            "Confirma que podes recompilar e que não guardaste ficheiros manuais aqui. Fecha builds e aplicações que usem os binários. Prefere cargo clean no projeto; a próxima compilação será mais lenta e pode precisar de rede.",
        )
    } else if name == ".next" && next_output(scan, index) {
        (
            "Saída de compilação Next.js",
            "Existe package.json na pasta superior e foram encontrados server, build-manifest.json e um marcador de build dentro de .next.",
            "Confirma que é um projeto de desenvolvimento parado, que só contém dados gerados e que podes voltar a compilar. next start usa estes ficheiros para servir a aplicação; apagá-los pode parar um site em produção.",
        )
    } else {
        return None;
    };
    let mut recommendation = Recommendation {
        node: index,
        title: title.into(),
        reason: reason.into(),
        caution: caution.into(),
        category: Category::Regenerable,
        bytes: node.bytes,
        action: Action::Inspect,
    };
    if context.cloud {
        recommendation.category = Category::Review;
        recommendation.caution.push_str(" A localização sugere sincronização cloud: eliminar pode propagar-se à cloud e a outros dispositivos. Revê a sincronização e prefere a opção de libertar espaço do respetivo cliente.");
    }
    if node.incomplete {
        recommendation.category = Category::Review;
        recommendation
            .reason
            .push_str(" A leitura deste ramo ficou incompleta.");
        recommendation.caution.push_str(" Há itens sem acesso ou por analisar: completa a análise e revê todo o conteúdo antes de decidir.");
    }
    Some(recommendation)
}

fn windows_cleanup(index: usize, node: &Node, location: Location) -> Option<Recommendation> {
    if !node.directory || node.bytes == 0 {
        return None;
    }
    let (title, reason, caution, action) = match location {
        Location::Temp if node.bytes >= 16 * MIB => (
            "Temporários: usar a limpeza do Windows",
            "Foi encontrada uma localização padrão de temporários do Windows com dados medidos.",
            "Usa as recomendações de limpeza do Windows e revê as categorias. Não apagues a pasta inteira manualmente: pode conter ficheiros em uso.",
            Action::StorageSettings,
        ),
        Location::Recycle => (
            "Rever a Reciclagem",
            "Foram medidos itens dentro da Reciclagem desta unidade.",
            "Abre a Reciclagem e confirma que não precisas de restaurar nada. Esvaziá-la elimina definitivamente o conteúdo; esta análise não faz essa ação.",
            Action::RecycleBin,
        ),
        Location::OldWindows if node.bytes >= 16 * MIB => (
            "Instalação anterior do Windows",
            "Foi encontrada Windows.old na raiz da unidade.",
            "Utiliza a limpeza do Windows, não a eliminação direta. Remover a instalação anterior pode impedir voltar à versão anterior do Windows.",
            Action::StorageSettings,
        ),
        _ => return None,
    };
    let mut caution = caution.to_owned();
    if node.incomplete {
        caution.push_str(" A análise deste ramo é parcial; o valor apresentado inclui apenas os bytes que foi possível medir.");
    }
    Some(Recommendation {
        node: index,
        title: title.into(),
        reason: reason.into(),
        caution,
        category: Category::WindowsCleanup,
        bytes: node.bytes,
        action,
    })
}

fn personal_review(
    index: usize,
    node: &Node,
    context: Context,
    now: i64,
) -> Option<Recommendation> {
    if node.directory || !context.personal {
        return None;
    }
    let age = node
        .details
        .modified
        .filter(|date| *date > 0 && *date <= now)
        .map(|date| now - date);
    let old_download =
        context.downloads && node.bytes >= 128 * MIB && age.is_some_and(|days| days >= 90 * DAY);
    let large_file = node.bytes >= 512 * MIB;
    if !old_download && !large_file {
        return None;
    }
    let reason = if old_download {
        "Está numa pasta de transferências, tem pelo menos 128 MiB e não foi modificado há pelo menos 90 dias. Isso não prova que deixou de ser necessário."
    } else {
        "É um ficheiro com pelo menos 512 MiB numa localização habitualmente pessoal. O tamanho, por si só, não indica que pode ser removido."
    };
    let mut caution = "Não é lixo identificado. Confirma o conteúdo, a utilidade e se é uma cópia única ou um backup. A data de acesso não é usada como prova de desuso.".to_owned();
    if context.cloud {
        caution.push_str(" A localização sugere sincronização cloud: eliminar pode remover também a cópia online. Para libertar apenas espaço local, usa o cliente de sincronização.");
    }
    Some(Recommendation {
        node: index,
        title: if old_download {
            "Transferência antiga para rever"
        } else {
            "Ficheiro pessoal volumoso"
        }
        .into(),
        reason: reason.into(),
        caution,
        category: Category::Review,
        bytes: node.bytes,
        action: Action::Inspect,
    })
}

#[derive(Debug)]
struct Ranked(Recommendation);
impl Ranked {
    fn key(&self) -> (u8, u64, Reverse<usize>) {
        let priority = match self.0.category {
            Category::WindowsCleanup => 3,
            Category::Regenerable => 2,
            Category::Review => 1,
        };
        (priority, self.0.bytes, Reverse(self.0.node))
    }
}
impl PartialEq for Ranked {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}
impl Eq for Ranked {}
impl PartialOrd for Ranked {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ranked {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

/// O(nodes + edges), apart from a heap whose capacity is fixed at 200. Contexts
/// are inherited by index; no full path is built for each file or deep recursion.
pub fn analyze(scan: &Scan, now_unix: i64) -> Report {
    let mut report = Report {
        incomplete: scan.cancelled
            || scan.progress.errors > 0
            || scan.nodes.first().is_some_and(|n| n.incomplete),
        ..Default::default()
    };
    if scan.nodes.is_empty() {
        return report;
    }
    let root = Context::root(scan);
    let mut contexts = Vec::<Context>::with_capacity(scan.nodes.len());
    let mut best: BinaryHeap<Reverse<Ranked>> = BinaryHeap::with_capacity(MAX_ITEMS + 1);
    for (index, node) in scan.nodes.iter().enumerate() {
        if !node.directory {
            report.inspected_files += 1;
        }
        let name = node.name.to_string_lossy().to_lowercase();
        let mut context = if index == 0 {
            root
        } else {
            contexts[node.parent.expect("scanned children have a parent")]
                .child(&name, node.directory)
        };
        let candidate = if context.covered || node.bytes == 0 {
            None
        } else if let Some(item) = windows_cleanup(index, node, context.location) {
            Some(item)
        } else if context.blocked
            || (!node.directory
                && (node.details.attributes & (0x4 | 0x400 | 0x1000) != 0
                    || matches!(
                        name.as_str(),
                        "pagefile.sys" | "hiberfil.sys" | "swapfile.sys"
                    )))
        {
            None
        } else {
            generated(scan, index, context, &name)
                .or_else(|| personal_review(index, node, context, now_unix))
        };
        if let Some(candidate) = candidate {
            // The first candidate in a branch owns its measured bytes, even when
            // the bounded display list later omits it in favour of a larger one.
            context.covered = node.directory;
            best.push(Reverse(Ranked(candidate)));
            if best.len() > MAX_ITEMS {
                best.pop();
                report.truncated = true;
            }
        }
        contexts.push(context);
    }
    let mut ranked: Vec<_> = best.into_iter().map(|Reverse(item)| item).collect();
    ranked.sort_unstable_by(|a, b| b.cmp(a));
    report.items = ranked.into_iter().map(|item| item.0).collect();
    for item in &report.items {
        match item.category {
            Category::Regenerable => report.regenerable_bytes += item.bytes,
            Category::Review => report.review_bytes += item.bytes,
            Category::WindowsCleanup => {}
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{platform::FileDetails, scanner::Progress};
    use std::{collections::HashMap, path::PathBuf};
    const NOW: i64 = 1_800_000_000;
    struct Fixture {
        scan: Scan,
    }
    impl Fixture {
        fn new(root: &str) -> Self {
            Self {
                scan: Scan {
                    root: PathBuf::from(root),
                    nodes: vec![Node {
                        name: root.into(),
                        parent: None,
                        children: vec![],
                        directory: true,
                        bytes: 0,
                        files: 0,
                        folders: 0,
                        details: FileDetails::default(),
                        incomplete: false,
                    }],
                    progress: Progress::default(),
                    cancelled: false,
                    issues: vec![],
                    extensions: HashMap::new(),
                    workers: 1,
                },
            }
        }
        fn add(&mut self, parent: usize, name: &str, directory: bool, bytes: u64) -> usize {
            let index = self.scan.nodes.len();
            self.scan.nodes.push(Node {
                name: name.into(),
                parent: Some(parent),
                children: vec![],
                directory,
                bytes,
                files: u64::from(!directory),
                folders: 0,
                details: FileDetails::default(),
                incomplete: false,
            });
            self.scan.nodes[parent].children.push(index);
            index
        }
        fn cargo(&mut self, parent: usize, bytes: u64) -> usize {
            self.add(parent, "Cargo.toml", false, 1);
            let target = self.add(parent, "target", true, bytes);
            self.add(target, ".rustc_info.json", false, 1);
            self.add(target, "CACHEDIR.TAG", false, 1);
            self.add(target, "debug", true, bytes - 2);
            target
        }
    }

    #[test]
    fn generated_artifacts_require_manifest_and_output_markers() {
        let mut f = Fixture::new(r"C:\Projects");
        let fake = f.add(0, "target", true, 300 * MIB);
        f.add(fake, "family.zip", false, 300 * MIB);
        let project = f.add(0, "real-project", true, 100 * MIB);
        let real = f.cargo(project, 100 * MIB);
        let report = analyze(&f.scan, NOW);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].node, real);
        assert_eq!(report.items[0].category, Category::Regenerable);
        assert_eq!(report.items[0].action, Action::Inspect);
    }

    #[test]
    fn unexpected_files_in_target_and_dependency_trees_are_not_regenerable() {
        let mut f = Fixture::new(r"C:\Users\me\Documents");
        let target = f.cargo(0, 600 * MIB);
        f.add(target, "manual-backup.zip", false, 1);
        let dependency = f.add(0, "node_modules", true, 900 * MIB);
        f.cargo(dependency, 900 * MIB);
        let git = f.add(0, ".git", true, 900 * MIB);
        f.add(git, "archive.zip", false, 900 * MIB);
        assert!(analyze(&f.scan, NOW).items.is_empty());
    }

    #[test]
    fn system_files_are_never_review_candidates_even_in_personal_folders() {
        let mut f = Fixture::new(r"C:\Users\me\Downloads");
        f.add(0, "pagefile.sys", false, 900 * MIB);
        let system = f.add(0, "system.bin", false, 900 * MIB);
        f.scan.nodes[system].details.attributes = 4;
        let app = f.add(0, "Program Files", true, 900 * MIB);
        f.add(app, "payload.zip", false, 900 * MIB);
        assert!(analyze(&f.scan, NOW).items.is_empty());
        let mut windows = Fixture::new(r"C:\Windows\Installer");
        windows.add(0, "installer.msi", false, 900 * MIB);
        assert!(analyze(&windows.scan, NOW).items.is_empty());
    }

    #[test]
    fn next_requires_build_evidence_and_rejects_unexpected_content() {
        let mut f = Fixture::new(r"C:\Projects\web");
        f.add(0, "package.json", false, 1);
        let next = f.add(0, ".next", true, 100 * MIB);
        assert!(analyze(&f.scan, NOW).items.is_empty());
        f.add(next, "server", true, 100 * MIB - 2);
        f.add(next, "build-manifest.json", false, 1);
        f.add(next, "BUILD_ID", false, 1);
        let report = analyze(&f.scan, NOW);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].category, Category::Regenerable);
        assert!(report.items[0].caution.contains("produção"));
        f.add(next, "private-backup.zip", false, 1);
        assert!(analyze(&f.scan, NOW).items.is_empty());
    }

    #[test]
    fn windows_temp_is_delegated_and_its_children_are_not_double_counted() {
        let mut f = Fixture::new(r"C:\Users\me\AppData\Local");
        let temp = f.add(0, "Temp", true, 800 * MIB);
        f.add(temp, "large.zip", false, 800 * MIB);
        let report = analyze(&f.scan, NOW);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].node, temp);
        assert_eq!(report.items[0].category, Category::WindowsCleanup);
        assert_eq!(report.items[0].action, Action::StorageSettings);
        assert_eq!((report.regenerable_bytes, report.review_bytes), (0, 0));
    }

    #[test]
    fn ages_are_only_review_evidence_and_unknown_or_future_dates_are_not_old() {
        let mut f = Fixture::new(r"D:\Downloads");
        let old = f.add(0, "old-installer.exe", false, 150 * MIB);
        f.scan.nodes[old].details.modified = Some(NOW - 100 * DAY);
        f.add(0, "unknown.zip", false, 150 * MIB);
        let future = f.add(0, "future.zip", false, 150 * MIB);
        f.scan.nodes[future].details.modified = Some(NOW + DAY);
        let large = f.add(0, "large-unique-video.mp4", false, 600 * MIB);
        let report = analyze(&f.scan, NOW);
        assert_eq!(report.items.len(), 2);
        assert!(
            report
                .items
                .iter()
                .all(|item| item.category == Category::Review && item.action == Action::Inspect)
        );
        assert_eq!(report.review_bytes, 750 * MIB);
        assert!(report.items.iter().any(|item| item.node == old));
        assert!(report.items.iter().any(|item| item.node == large));
    }

    #[test]
    fn incomplete_or_cloud_builds_are_review_only() {
        let mut partial = Fixture::new(r"C:\Projects");
        let target = partial.cargo(0, 100 * MIB);
        partial.scan.nodes[target].incomplete = true;
        partial.scan.nodes[0].incomplete = true;
        let report = analyze(&partial.scan, NOW);
        assert!(report.incomplete);
        assert_eq!(report.regenerable_bytes, 0);
        assert_eq!(report.items[0].category, Category::Review);
        let mut cloud = Fixture::new(r"C:\Users\me\OneDrive - Company\Project");
        cloud.cargo(0, 100 * MIB);
        let report = analyze(&cloud.scan, NOW);
        assert_eq!(report.items[0].category, Category::Review);
        assert!(report.items[0].caution.contains("cloud"));
    }

    #[test]
    fn nested_candidates_never_overlap_and_empty_scans_are_supported() {
        let mut f = Fixture::new(r"C:\Users\me\Documents");
        let target = f.cargo(0, 800 * MIB);
        let debug = f.scan.nodes[target]
            .children
            .iter()
            .copied()
            .find(|i| f.scan.nodes[*i].directory)
            .unwrap();
        f.add(debug, "important-looking.mp4", false, 800 * MIB);
        let report = analyze(&f.scan, NOW);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.regenerable_bytes, 800 * MIB);
        assert_eq!(report.review_bytes, 0);
        f.scan.nodes.clear();
        assert!(analyze(&f.scan, NOW).items.is_empty());
    }

    #[test]
    fn large_metadata_sets_have_bounded_results_and_keep_largest_candidates() {
        let mut f = Fixture::new(r"C:\Users\me\Videos");
        for i in 0..10_000 {
            f.add(0, &format!("video-{i}.mp4"), false, (600 + i) * MIB);
        }
        let report = analyze(&f.scan, NOW);
        assert_eq!(report.inspected_files, 10_000);
        assert_eq!(report.items.len(), MAX_ITEMS);
        assert!(report.truncated);
        assert_eq!(report.items[0].bytes, 10_599 * MIB);
        assert_eq!(report.items.last().unwrap().bytes, 10_400 * MIB);
        assert_eq!(
            report.review_bytes,
            report.items.iter().map(|i| i.bytes).sum::<u64>()
        );
    }
}
