// Sem janela de console no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Modo daemon: a UI reexecuta ESTE mesmo binário com `--daemon` para subir o
    // servidor de sessões (sem sidecar → empacotamento trivial). Instâncias extras
    // saem sozinhas ao perder o lock (single-instance).
    if std::env::args().any(|a| a == "--daemon") {
        let config = perene_daemon::Config::from_env();
        if let Err(e) = perene_daemon::run(config) {
            let msg = e.to_string();
            if !msg.contains("lock ocupado") {
                eprintln!("perene-daemon: {msg}");
                std::process::exit(1);
            }
        }
        return;
    }

    perene_desktop_lib::run();
}
