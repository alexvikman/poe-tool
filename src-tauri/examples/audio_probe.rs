//! Prints every audio session as `pid volume`. For testing on Windows:
//! `cargo run --example audio_probe`
fn main() {
    match poe_tool_lib::audio::list_sessions() {
        Ok(sessions) => {
            for (pid, volume) in sessions {
                println!("{pid} {volume:.3}");
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
