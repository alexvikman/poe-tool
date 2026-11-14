//! Prices an item from a text file (as copied with Ctrl+C in the game) and prints
//! the result as JSON: `cargo run --example price_probe -- item.txt [league]`
use poe_tool_lib::price::PriceService;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: price_probe <item.txt> [league]");
    let text = std::fs::read_to_string(path).expect("read item file");
    let cache = std::env::temp_dir().join("poe-tool-probe-cache");
    let service = PriceService::new(cache);
    let result = tauri::async_runtime::block_on(async {
        let league = match args.next() {
            Some(l) => l,
            None => service.default_league().await,
        };
        service.check(&text, &league).await
    });
    match result {
        Ok(check) => println!("{}", serde_json::to_string_pretty(&check).unwrap()),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
