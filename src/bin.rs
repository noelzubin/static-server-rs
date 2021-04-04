use static_server::Server;

fn main() {
    Server::builder()
        .allow_ext(&["png", "svg", "jpeg", "jpg"])
        .prefix("/local")
        .root("/")
        .run();
}
