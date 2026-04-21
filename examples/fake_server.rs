// Stand-in for ArmaReforgerServer.exe used by integration tests.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(out) = std::env::var("FAKE_SERVER_ARGS_OUT") {
        let _ = std::fs::write(&out, args.join("\n"));
    }
    std::process::exit(0);
}
