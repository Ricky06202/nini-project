use nini_compiler::{generate_component_js, parse_component};
use std::fs;
use std::path::Path;
use std::thread;
use tiny_http::{Response, Server};

const HTML_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html lang="es">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Nini App</title>
    <link rel="stylesheet" href="styles.css">
    <style>
        body { font-family: sans-serif; background: #1a1a1a; color: white; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; }
        #nini-app { border: 1px solid #333; padding: 2rem; border-radius: 8px; background: #252525; box-shadow: 0 4px 15px rgba(0,0,0,0.5); }
    </style>
</head>
<body>
    <div id="nini-app">
        <h1>Cargando Nini...</h1>
    </div>
    <script type="module" src="./bundle.js"></script>
</body>
</html>
"#;

fn main() {
    let ruta_entrada = "samples/Contador.nini";
    let ruta_salida_js = "dist/bundle.js";
    let ruta_salida_html = "dist/index.html";

    let contenido = fs::read_to_string(ruta_entrada).expect("Error al leer");

    // 1. Parsear (Fase 1)
    if let Ok((_, component)) = parse_component(&contenido) {
        // Generar un ID de scope único (por ahora simple)
        let scope_class = "nini-1";
        // 2. Generar JS y CSS (Fase 2 y 2.5)
        let (js_final, css_final) = generate_component_js(&component, scope_class);

        // 3. Crear directorio dist y subdirectorio para runtime
        fs::create_dir_all("dist/nini-runtime-web").ok();

        // 4. Copiar el runtime de Nini
        let runtime_src = "packages/nini-runtime-web/core.js";
        let runtime_dst = "dist/nini-runtime-web/core.js";
        fs::copy(runtime_src, runtime_dst).expect("Error al copiar runtime");

        // 5. Escribir archivo JS
        fs::write(ruta_salida_js, &js_final).expect("Error al escribir JS");
        // 6. Escribir archivo CSS
        let ruta_salida_css = "dist/styles.css";
        fs::write(ruta_salida_css, &css_final).expect("Error al escribir CSS");

        // 7. Escribir el HTML (El Shell de Nini)
        fs::write(ruta_salida_html, HTML_TEMPLATE).expect("Error al escribir HTML");

        println!("🚀 Nini Build Exitosa:");
        println!("   - JS: dist/bundle.js");
        println!("   - HTML: dist/index.html");

        // 6. Iniciar servidor local
        let server = Server::http("0.0.0.0:8080").unwrap();
        println!("\n🌐 Servidor local en http://localhost:8080");
        println!("   Presiona Ctrl+C para detener.");

        // Abrir navegador
        thread::spawn(|| {
            open::that("http://localhost:8080").ok();
        });

        // Manejar requests
        for request in server.incoming_requests() {
            let url = request.url();
            let response = if url == "/" {
                let html = fs::read_to_string(ruta_salida_html)
                    .unwrap_or_else(|_| HTML_TEMPLATE.to_string());
                Response::from_string(html).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap(),
                )
            } else {
                // Servir archivos estáticos desde dist/
                let path = url.trim_start_matches('/');
                let file_path = format!("dist/{}", path);
                if let Ok(content) = fs::read(&file_path) {
                    let content_type =
                        match Path::new(&file_path).extension().and_then(|e| e.to_str()) {
                            Some("js") => "application/javascript",
                            Some("html") => "text/html",
                            Some("css") => "text/css",
                            _ => "application/octet-stream",
                        };
                    Response::from_data(content).with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            content_type.as_bytes(),
                        )
                        .unwrap(),
                    )
                } else {
                    Response::from_string("404 Not Found").with_status_code(404)
                }
            };
            request.respond(response).unwrap();
        }
    } else {
        println!("❌ Error de sintaxis en el archivo Nini");
    }
}
