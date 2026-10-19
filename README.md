[goat.anti.run](https://goat.anti.run/)

Run the server with `cargo run --release -p goat_server`. Bots use the selected
war search and learned rummy strategy by default for one-deck games, with weights
bundled into the binary. Multi-deck games use the original `AdaptSimulate` bot.

[Training and evaluation](training/README.md)
