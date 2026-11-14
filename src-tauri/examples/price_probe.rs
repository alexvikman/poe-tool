//! Prices an item from a text file (as copied with Ctrl+C in the game) and prints
//! the result as JSON: `cargo run --example price_probe -- item.txt [league]`.
//! `-- --gem <name> [league]` searches transfigured gems instead.
use poe_tool_lib::price::PriceService;

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    let gem = args.peek().is_some_and(|a| a == "--gem");
    if gem {
        args.next();
    }
    let input = args.next().expect("usage: price_probe <item.txt> [league] | --gem <name> [league]");
    let cache = std::env::temp_dir().join("poe-tool-probe-cache");
    let service = PriceService::new(cache);
    let league = args.next();
    let json = tauri::async_runtime::block_on(async {
        let league = match league {
            Some(l) => l,
            None => service.default_league().await,
        };
        if gem {
            service.gem_search(&input, &league).await.map(|r| serde_json::to_string_pretty(&r).unwrap())
        } else {
            let text = std::fs::read_to_string(&input).expect("read item file");
            service.check(&text, &league, true).await.map(|r| serde_json::to_string_pretty(&r).unwrap())
        }
    });
    match json {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
