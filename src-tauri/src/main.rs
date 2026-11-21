fn main() {
    // CLI 子命令（generate / providers / --help）：无头模式，不启动 GUI、不监听任何端口。
    // 不带任何参数时启动图形界面。
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if let Err(err) = xiic_image_studio_lib::cli::run(args) {
            eprintln!("[cli] {err:#}");
            std::process::exit(1);
        }
        return;
    }
    xiic_image_studio_lib::run()
}
