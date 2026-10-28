# SpaceFinder

Explorador de espaço em disco, nativo em Rust, para Windows. Interface em português, sem servidor, browser ou runtime JavaScript.

## Interface e recomendações (0.4)

Nova navegação lateral com **Visão geral**, **Explorador**, **Maiores ficheiros**, **Tipos de ficheiro** e **Recomendações**. O resumo distingue o tamanho lógico analisado da ocupação física da unidade e apresenta as maiores pastas. Tabelas, cabeçalhos, propriedades, seleção e mapa foram redesenhados; as recomendações têm uma lista filtrável e um painel com evidência, cautelas e ações de revisão.

O motor de recomendações usa os metadados já recolhidos e corre em segundo plano, sem ler o conteúdo dos ficheiros nem apagar dados. Cada relatório fica ligado à análise que o originou e é descartado ao iniciar outra.

- **Gerir pelo Windows:** temporários em localizações padrão, Reciclagem e `Windows.old` quando encontrados. A ação abre a ferramenta do Windows para revisão; não executa uma limpeza.
- **Pode ser recriado:** saídas de compilação Cargo e Next.js com manifesto e marcadores de saída correspondentes. Exige confirmar que não existem ficheiros manuais, que o projeto pode ser recompilado e que nenhum build/servidor está a usar os ficheiros. Builds podem conter binários finais. Uma pasta chamada `target` ou `.next`, por si só, não basta.
- **Revisão pessoal:** ficheiros com pelo menos 512 MiB em localizações pessoais e transferências com pelo menos 128 MiB, modificadas há pelo menos 90 dias. Tamanho e idade não provam que um ficheiro pode ser eliminado. A data de último acesso não é usada como prova de desuso.

Pastas de sistema, instalações geridas, `.git` e `node_modules` são excluídas de sugestões de eliminação direta. Cloud e ramos incompletos exigem revisão adicional. As sugestões não identificam duplicados sem comparação de conteúdo. Os tamanhos são lógicos e evitam dupla contagem entre uma pasta recomendada e os seus descendentes. São apresentadas até 200 sugestões prioritárias; os totais representam as sugestões efetivamente listadas.

**As recomendações não fazem limpeza automática.** O botão **Rever na árvore** seleciona o item para inspeção, sem o executar. A eliminação continua no menu explícito do explorador, com confirmação. As ferramentas Windows são abertas para o utilizador escolher o que limpar; não se alteram permissões nem opções do sistema automaticamente.

```powershell
.\dist\SpaceFinder-CLI.exe --scan 'C:\' --json --recommendations
```

Com `--recommendations`, a CLI inclui caminhos, categorias, tamanhos observados, motivos e cautelas no JSON. O parâmetro também funciona na apresentação textual.

Referências dos critérios: [Microsoft — libertar espaço](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows), [Storage Sense](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/manage-drive-space-with-storage-sense), [Cargo clean](https://doc.rust-lang.org/cargo/commands/cargo-clean.html), [Next.js — saída da compilação](https://nextjs.org/docs/app/api-reference/config/next-config-js/distDir).

## Permissões e ações (0.3)

**Administrador é opcional.** A app funciona em modo normal; **Opções → Abrir como administrador** pede autorização ao Windows e abre uma nova janela com o mesmo caminho. Quando essa janela confirma que abriu com privilégios de administrador e já desenhou a interface, a anterior fecha automaticamente. Se cancelares o pedido, a nova instância falhar ou não confirmar a abertura, a janela atual permanece aberta. Pode permitir analisar mais pastas. Algumas localizações, como partes de `System Volume Information` e `WindowsApps`, podem continuar protegidas. Não são alteradas permissões nem proprietários de ficheiros.

A barra de estado distingue pastas sem acesso, outros erros e ligações ignoradas. Uma faixa avisa quando a análise está incompleta. Os ramos afetados têm um indicador e `*` junto ao tamanho: o valor é apenas o que foi possível medir, não zero confirmado. A janela **Cobertura e permissões** permite filtrar as primeiras 100 ocorrências, copiar um relatório e consultar as explicações de tamanhos e datas.

O menu de contexto da tabela e do mapa inclui:

- Abrir o item, mostrar/selecionar no Explorador, abrir a pasta superior e propriedades nativas do Windows.
- Focar, expandir/recolher ou analisar uma pasta novamente.
- Copiar nome, caminho, caminho da pasta superior e bytes exatos.
- **Eliminar para a Reciclagem**, com confirmação dentro da app; para pastas, é necessário escrever o nome.

As ações de ficheiros correm em segundo plano. A raiz da análise e as unidades não podem ser eliminadas através deste menu. A app verifica o caminho absoluto, os limites da análise, ligações e alterações ao item antes da operação. A reciclagem está limitada a unidades locais fixas e usa a operação nativa do Windows, sem uma função alternativa de eliminação permanente. O Windows pode recusar ou cancelar a ação.

Depois de reciclar, a app volta a analisar e retoma a pasta superior. **O espaço livre pode não aumentar até a Reciclagem ser esvaziada.** O CSV passa a ter uma décima coluna, `incomplete`, e a CLI JSON apresenta `denied` nas estatísticas e erros estruturados com caminho, motivo e categoria.

Referências: [Shell do Windows e administrador](https://learn.microsoft.com/en-us/windows/win32/shell/launch) e [operações nativas de ficheiros](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-setoperationflags).

## Vista detalhada (0.2)

- Árvore expansível com hierarquia, tamanhos e barras de percentagem da pasta superior. Clica na seta ou faz duplo clique para expandir/recolher.
- Colunas redimensionáveis; ordenação por nome, tamanho, ficheiros, subpastas, última alteração e último acesso. Clica novamente no cabeçalho para inverter a ordem.
- Colunas adicionais de criação, acesso, atributos e caminho completo no menu **Colunas**. Deslocação horizontal quando a tabela não cabe na janela.
- Painel de propriedades com bytes exatos, conteúdo do ramo, itens diretos, média por ficheiro, percentagem da pasta e da análise, datas, atributos e caminho.
- Vista **Tipos de ficheiro**, agrupada por extensão, com contagens, tamanho total/médio e percentagens. Duplo clique numa extensão abre os maiores ficheiros correspondentes.
- Pesquisa em todos os níveis da árvore, mantendo visíveis os antepassados dos resultados. Os filtros alteram as linhas visíveis; os totais continuam a representar a análise original.
- Níveis de expansão 1/2/3, vista só de pastas, foco de um ramo e histórico de navegação.
- Unidades Auto/Bytes/KiB/MiB/GiB, linhas compactas e painéis de mapa/propriedades opcionais e redimensionáveis.
- **Ctrl+F** para pesquisar, **F5** para reanalisar e **Esc** para parar.

As datas e os atributos vêm da mesma enumeração nativa, sem uma chamada extra por ficheiro. As datas de alteração e acesso dos ramos são a mais recente entre a pasta e todos os descendentes analisados; a criação é a da própria pasta. O Windows pode atrasar ou desativar a atualização dos acessos. O CSV inclui também as datas Unix em UTC (segundos) e os atributos numéricos do Windows. Os ficheiros JSON da CLI incluem estes detalhes nas maiores entradas.

Referências: [colunas do TreeSize](https://manuals.jam-software.com/treesize_free/EN/available_columns.html) e [metadados WIN32_FIND_DATAW](https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-win32_find_dataw).

## Abrir

O executável portátil está em `dist/SpaceFinder.exe` depois de executar `./build.ps1`. Pode ser aberto por duplo clique. Escolhe uma unidade, escreve um caminho ou usa **Escolher pasta**.

```powershell
cargo run --release
cargo run --release -- 'C:\Users'
```

- Análise de pastas/unidades em segundo plano, com estatísticas de progresso.
- Pastas e ficheiros ordenados por tamanho, mapa de ocupação e duplo clique para navegar.
- Vista dos 1 000 maiores ficheiros, filtro por nome e menu de contexto para copiar caminhos/abrir o Explorador.
- Cancelamento com resultados parciais (botão ou Esc).
- Exportação CSV em UTF-8, incluindo todos os itens e os totais por pasta.
- Informação de capacidade e espaço livre nas unidades locais fixas.

## Motor de análise

No Windows, usa `FindFirstFileExW` com `FindExInfoBasic` e `FIND_FIRST_EX_LARGE_FETCH`, seguido de `FindNextFileW`. Os tamanhos vêm da própria enumeração: não abre cada ficheiro nem lê o seu conteúdo. Várias threads enumeram pastas em paralelo. As filas de resultados têm capacidade limitada, os lotes têm no máximo 1 024 entradas e os totais são agregados numa passagem O(n). A lista da interface é virtualizada.

Referência da API: [Microsoft — FindFirstFileExW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfileexw). Medições e verificações locais estão em [VALIDATION.md](VALIDATION.md).

O número de workers é ajustável. O valor inicial depende do número de CPUs, limitado a 16; SSDs costumam beneficiar de paralelismo e HDDs podem beneficiar de 1–2 workers. O resultado depende do disco, da cache, do antivírus, do número de pastas e das permissões. Não há leitura direta da MFT nem necessidade de administrador.

## Precisão e cobertura

Os tamanhos são **lógicos**, em bytes (unidades binárias KiB/MiB/GiB), e não o espaço físico alocado. Ficheiros comprimidos/sparse podem ocupar menos espaço. Hard links são contados por caminho. A capacidade/espaço livre da unidade vem do Windows e pode, por isso, diferir dos totais enumerados.

Inclui entradas ocultas e de sistema quando há permissão. Ignora reparse points, incluindo junctions, links simbólicos e alguns placeholders cloud, para evitar ciclos e atravessar outros volumes. Mostra contagem de itens ignorados e erros, com os primeiros 100 erros disponíveis no rodapé. Pastas sem permissão e uma análise interrompida produzem resultados parciais. O disco pode mudar durante a leitura; isto não é um snapshot transacional.

A análise lê apenas metadados. As ações explícitas do menu podem abrir itens ou enviá-los para a Reciclagem após confirmação.

## Linha de comandos / medição

```powershell
.\dist\SpaceFinder-CLI.exe --scan 'C:\Users' --threads 8
.\dist\SpaceFinder-CLI.exe --scan 'C:\Users' --threads 8 --json
```

O JSON contém caminho, workers, bytes, ficheiros, pastas, segundos, erros, itens ignorados, extensões e as 30 maiores entradas da raiz. Compara workers no mesmo caminho e considera o efeito da cache; uma análise de uma pasta não prevê o tempo do disco inteiro.

## Compilar e testar

Requer Rust estável e as ferramentas de compilação MSVC para Windows.

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
./build.ps1
```

O scanner tem também uma implementação portátil baseada em `read_dir` para outros sistemas. A versão entregue e a integração com o Explorador são destinadas a Windows.

