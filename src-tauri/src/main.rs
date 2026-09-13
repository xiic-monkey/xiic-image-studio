fn main() {
    // `--mcp`：无头模式。不启动 GUI、不监听任何端口——
    // agent 的 MCP 客户端把这个可执行文件当子进程拉起，JSON-RPC 走 stdin/stdout。
    if std::env::args().skip(1).any(|arg| arg == "--mcp") {
        if let Err(err) = xiic_image_studio_lib::mcp::run_stdio() {
            eprintln!("[mcp] {err:#}");
            std::process::exit(1);
        }
        return;
    }
    xiic_image_studio_lib::run()
}
