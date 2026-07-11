//! Binário do daemon. A UI o sobe automaticamente (detached) quando não há um
//! rodando; instâncias extras saem sozinhas ao falhar o lock (single-instance).

fn main() {
    let config = perene_daemon::Config::from_env();
    if let Err(e) = perene_daemon::run(config) {
        // Erro esperado e benigno: já existe um daemon (perdemos a corrida do
        // lock). Sai em silêncio com código 0 pra não poluir logs da UI.
        let msg = e.to_string();
        if msg.contains("lock ocupado") {
            std::process::exit(0);
        }
        eprintln!("perene-daemon: {msg}");
        std::process::exit(1);
    }
}
