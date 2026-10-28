fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args.first().is_some_and(|a| a == "--help" || a == "-h") {
        println!(
            "SpaceFinder CLI\n\nspacefinder-cli --scan PASTA [--threads 1..64] [--json] [--recommendations]"
        );
        return;
    }
    if args.first().is_none_or(|a| a != "--scan") {
        eprintln!(
            "Usa: spacefinder-cli --scan PASTA [--threads 1..64] [--json] [--recommendations]"
        );
        std::process::exit(1);
    }
    if let Err(error) = spacefinder::cli::run(&args) {
        eprintln!("Erro: {error}");
        std::process::exit(1);
    }
}
