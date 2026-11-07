//! The lazymkv-ocr worker binary (design part 10.7, K2): reads one PGS or VobSub stream of an evidence.mks into the
//! OCR evidence files under `--out`, printing nothing on standard output.

fn main() {
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(arg) => args.push(arg),
            Err(_) => {
                eprint!("an argument is not valid Unicode\n{}\n", lazymkv_ocr::stream::USAGE);
                std::process::exit(2);
            }
        }
    }
    std::process::exit(lazymkv_ocr::stream::main_with(&args, &mut std::io::stderr()));
}
