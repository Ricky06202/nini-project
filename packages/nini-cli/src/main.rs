use nini_compiler::{generate_component_js, parse_component_with_path, ComponentResolver};
use std::collections::HashMap;
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
    <link rel="stylesheet" href="/styles.css">
    <style>
        body { font-family: sans-serif; background: #1a1a1a; color: white; margin: 0; }
        #nini-app { min-height: 100vh; }
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

fn compile_page(page_path: &Path, base_dir: &str, page_name: &str) -> Option<(String, String)> {
    let contenido = fs::read_to_string(page_path).ok()?;

    if let Ok((_, mut component)) =
        parse_component_with_path(&contenido, page_path.to_string_lossy().to_string())
    {
        let mut resolver = ComponentResolver::new();
        if let Err(e) = resolver.resolve_imports(&mut component, base_dir) {
            println!("   ⚠️  Error de dependencias en {}: {}", page_name, e);
            return None;
        }

        let resolved_components = resolver.components;
        let scope_class = format!("nini-page-{}", page_name);

        let (js_code, css) = generate_component_js(&component, &scope_class, &resolved_components);
        Some((js_code, css))
    } else {
        println!("   ❌ Error de sintaxis en {}", page_name);
        None
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let modo = args.get(1).map(|s| s.as_str()).unwrap_or("dev");

    println!("🧭 Nini Router - Modo: {}", modo);

    let src_pages = "src/pages";
    let base_dir = "src";

    // Escanear páginas
    let mut routes: Vec<(String, String, String)> = Vec::new();
    let mut all_css = String::new();

    if Path::new(src_pages).exists() {
        println!("📂 Escaneando {}...", src_pages);

        if let Ok(entries) = fs::read_dir(src_pages) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "nini").unwrap_or(false) {
                    let file_name = path.file_stem().unwrap().to_string_lossy();
                    let page_name = file_name.to_string();

                    // Ruta: index.nini -> "/", otro -> "/nombre"
                    let route_path = if page_name == "index" {
                        "/".to_string()
                    } else {
                        format!("/{}", page_name)
                    };

                    println!("   📄 Página: {} -> {}", page_name, route_path);

                    // Usar el directorio de la página como base_dir
                    let page_dir = path
                        .parent()
                        .unwrap_or(Path::new("src/pages"))
                        .to_string_lossy()
                        .to_string();
                    if let Some((js_code, css)) = compile_page(&path, &page_dir, &page_name) {
                        // Corregir path de importación para páginas (están en subcarpeta)
                        let js_code = js_code.replace(
                            "from './nini-runtime-web/core.js'",
                            "from '../nini-runtime-web/core.js'",
                        );
                        // Guardar JS de la página
                        let js_file = format!("dist/pages/{}.js", page_name);
                        fs::create_dir_all("dist/pages").ok();
                        fs::write(&js_file, &js_code).ok();
                        routes.push((route_path, page_name.clone(), js_file));
                        all_css.push_str(&css);
                        all_css.push_str("\n");
                    }
                }
            }
        }
    } else {
        println!(
            "⚠️  No se encontró {}, compilando archivo único...",
            src_pages
        );

        let ruta_entrada = args
            .get(2)
            .map(|s| s.as_str())
            .unwrap_or("samples/Contador.nini");
        let contenido = fs::read_to_string(ruta_entrada).expect("Error al leer");

        let base_dir_single = Path::new(ruta_entrada)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "samples".to_string());

        if let Ok((_, mut component)) =
            parse_component_with_path(&contenido, ruta_entrada.to_string())
        {
            let mut resolver = ComponentResolver::new();
            resolver
                .resolve_imports(&mut component, &base_dir_single)
                .ok();

            let resolved_components = resolver.components;
            let scope_class = "nini-1";

            let (js_final, css_final) =
                generate_component_js(&component, scope_class, &resolved_components);

            fs::create_dir_all("dist/nini-runtime-web").ok();
            fs::copy(
                "packages/nini-runtime-web/core.js",
                "dist/nini-runtime-web/core.js",
            )
            .ok();
            fs::write("dist/bundle.js", &js_final).ok();
            all_css = css_final;

            routes.push((
                "/".to_string(),
                "index".to_string(),
                "bundle.js".to_string(),
            ));
        }
    }

    // Generar bundle principal con Router
    let router_js = generate_router_js(&routes);
    fs::write("dist/bundle.js", &router_js).ok();

    // Escribir CSS
    fs::write("dist/styles.css", &all_css).ok();

    // Copiar runtime
    fs::create_dir_all("dist/nini-runtime-web").ok();
    fs::copy(
        "packages/nini-runtime-web/core.js",
        "dist/nini-runtime-web/core.js",
    )
    .ok();

    // Escribir HTML
    fs::write("dist/index.html", HTML_TEMPLATE).ok();

    println!("\n🚀 Build Exitosa:");
    println!("   - Rutas: {}", routes.len());
    for (path, _, _) in &routes {
        println!("      {} -> {}", path, path);
    }
    println!("   - JS: dist/bundle.js");
    println!("   - HTML: dist/index.html");

    // Iniciar servidor
    let server = Server::http("0.0.0.0:8080").unwrap();
    println!("\n🌐 Servidor en http://localhost:8080");
    println!("   Presiona Ctrl+C para detener.\n");

    thread::spawn(|| {
        open::that("http://localhost:8080").ok();
    });

    // Rutas disponibles para el servidor
    let route_paths: Vec<String> = routes.iter().map(|(p, _, _)| p.clone()).collect();

    for request in server.incoming_requests() {
        let url = request.url();

        // SPA: cualquier ruta que no sea archivo va a index.html
        let is_file = url.contains('.') && !url.starts_with("/?");

        let response = if is_file {
            let path = url.trim_start_matches('/');
            let file_path = format!("dist/{}", path);
            if let Ok(content) = fs::read(&file_path) {
                let content_type = match Path::new(&file_path).extension().and_then(|e| e.to_str())
                {
                    Some("js") => "application/javascript",
                    Some("html") => "text/html",
                    Some("css") => "text/css",
                    _ => "application/octet-stream",
                };
                Response::from_data(content).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                        .unwrap(),
                )
            } else {
                Response::from_string("404 Not Found").with_status_code(404)
            }
        } else {
            // SPA: servir index.html para cualquier ruta
            let html =
                fs::read_to_string("dist/index.html").unwrap_or_else(|_| HTML_TEMPLATE.to_string());
            Response::from_string(html).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap(),
            )
        };

        request.respond(response).ok();
    }
}

fn generate_router_js(routes: &[(String, String, String)]) -> String {
    let mut js = String::new();

    js.push_str("import { nini } from './nini-runtime-web/core.js';\n\n");

    // Route map - embeber todo el JS de páginas directamente
    js.push_str("window.NINI_ROUTES = {\n");
    for (path, page_name, js_file) in routes {
        // Leer el JS de la página y embeberlo
        if let Ok(page_js) = fs::read_to_string(js_file) {
            // Eliminar el import y obtener el resto del código
            let code_without_import: String = page_js
                .lines()
                .filter(|line| !line.starts_with("import "))
                .collect::<Vec<_>>()
                .join("\n");

            js.push_str(&format!(
                "    '{}': {{ page: '{}', render: () => {{ {} }} }},\n",
                path, page_name, code_without_import
            ));
        }
    }
    js.push_str("};\n\n");

    // Router class
    js.push_str(
        r#"
class NiniRouter {
    constructor() {
        this.currentRoute = null;
        this.init();
    }
    
    init() {
        // Interceptar clics en enlaces
        document.addEventListener('click', (e) => {
            const link = e.target.closest('[data-nini-link]');
            console.log('🖱️ Click detectado:', e.target, link);
            if (link) {
                e.preventDefault();
                const href = link.getAttribute('href');
                console.log('🧭 Navegando a:', href);
                this.navigate(href);
            }
        });
        
        // Manejar botón atrás/adelante
        window.addEventListener('popstate', () => {
            this.loadRoute(window.location.pathname);
        });
        
        // Cargar ruta inicial
        this.loadRoute(window.location.pathname);
    }
    
    async navigate(path) {
        window.history.pushState({}, '', path);
        await this.loadRoute(path);
    }
    
    async loadRoute(path) {
        const route = window.NINI_ROUTES?.[path];
        
        console.log('📍 Cargando ruta:', path);
        
        if (!route) {
            console.warn('Ruta no encontrada:', path);
            return;
        }
        
        // Ejecutar la función render embebida
        if (route.render) {
            route.render();
            console.log('✅ Página renderizada:', route.page);
            this.currentRoute = path;
        }
    }
}

// Inicializar router inmediatamente
try {
    console.log('🚀 Nini Router inicializado');
    window.niniRouter = new NiniRouter();
} catch (e) {
    console.error('❌ Error al inicializar router:', e);
}
"#,
    );

    js
}
