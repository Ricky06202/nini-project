// packages/nini-compiler/src/lib.rs

#[derive(Debug, Clone, PartialEq)]
pub enum NiniNode {
    Variable {
        name: String,
        value: String,
    },
    Function {
        name: String,
        body: Vec<NiniNode>,
    },
    Class {
        name: String,
        members: Vec<NiniNode>,
    },
    HtmlElement {
        tag: String,
        attributes: Vec<(String, String)>,
        content: String,
    },
    Expression(String),
    Import {
        path: String,
        alias: String,
    },
}

#[derive(Debug)]
pub struct AbstractSyntaxTree {
    pub nodes: Vec<NiniNode>,
}

#[derive(Debug)]
enum TemplatePart {
    Text(String),
    Expression(String),
}

fn parse_template(template: &str) -> Vec<TemplatePart> {
    let mut parts = Vec::new();
    let mut current = template;
    while !current.is_empty() {
        if let Some(start) = current.find('{') {
            // Texto antes de la llave
            if start > 0 {
                parts.push(TemplatePart::Text(current[..start].to_string()));
            }
            // Encontrar la llave de cierre
            if let Some(end) = current[start..].find('}') {
                let expr = &current[start + 1..start + end];
                parts.push(TemplatePart::Expression(expr.trim().to_string()));
                current = &current[start + end + 1..];
            } else {
                // No hay cierre, tratar el resto como texto
                parts.push(TemplatePart::Text(current[start..].to_string()));
                break;
            }
        } else {
            // No más llaves
            parts.push(TemplatePart::Text(current.to_string()));
            break;
        }
    }
    parts
}

// Parser para un elemento HTML (genérico)
fn parse_html_element(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = space1(input)?; // indentación
    let (input, _) = tag("html:")(input)?;
    let (input, _) = line_ending(input)?;
    let (input, _) = space1(input)?; // indentación del contenido HTML
    let (input, html_line) = not_line_ending(input)?;
    let (input, _) = line_ending(input)?;

    let html = html_line.trim();

    // Extraer el tag name (entre < y > o espacio)
    let tag_start = html
        .find('<')
        .ok_or(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))?
        + 1;
    let tag_end = html[tag_start..]
        .find(|c| c == '>' || c == ' ')
        .ok_or(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )))?
        + tag_start;
    let tag = &html[tag_start..tag_end];

    // Extraer atributos (simplificado: buscamos on_click="...")
    let mut attributes = Vec::new();
    if let Some(on_click_start) = html.find("on_click=\"") {
        let after = &html[on_click_start + 10..];
        if let Some(quote_pos) = after.find('"') {
            let on_click_value = &after[..quote_pos];
            attributes.push(("on_click".to_string(), on_click_value.to_string()));
        }
    }

    // Extraer contenido entre > y </tag>
    let content_start = html.find('>').map(|pos| pos + 1).unwrap_or(html.len());
    let content_end = html.find("</").unwrap_or(html.len());
    let content = html[content_start..content_end].trim();

    Ok((
        input,
        NiniNode::HtmlElement {
            tag: tag.to_string(),
            attributes,
            content: content.to_string(),
        },
    ))
}

// Parser para una función
fn parse_function(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = space1(input)?; // Espera al menos un espacio/tab (indentación)
    let (input, _) = tag("fn ")(input)?;
    let (input, name) = alpha1(input)?;
    let (input, _) = tag("()")(input)?; // Por ahora funciones sin parámetros
    let (input, _) = line_ending(input)?;

    // Intentar parsear una línea de cuerpo (indentada)
    let (input, body) = if input.starts_with(' ') || input.starts_with('\t') {
        let (input, _) = space1(input)?; // indentación del cuerpo
        let (input, expr) = not_line_ending(input)?;
        let (input, _) = line_ending(input)?;
        (input, vec![NiniNode::Expression(expr.trim().to_string())])
    } else {
        (input, vec![])
    };

    Ok((
        input,
        NiniNode::Function {
            name: name.to_string(),
            body,
        },
    ))
}

// Parser para una variable (soporta strings entre comillas y valores literales)
fn parse_variable(input: &str) -> IResult<&str, NiniNode> {
    let (input, name) = alpha1(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("=")(input)?;
    let (input, _) = multispace0(input)?;

    // Intentar parsear un string entre comillas
    let (input, value) = if input.starts_with('"') {
        let (input, _) = tag("\"")(input)?;
        let (input, value) = take_while(|c: char| c != '"')(input)?;
        let (input, _) = tag("\"")(input)?;
        (input, value.to_string())
    } else {
        // Parsear cualquier valor hasta el final de línea
        let (input, value) = take_while(|c: char| c != '\n')(input)?;
        (input, value.trim().to_string())
    };

    let (input, _) = line_ending(input)?;
    Ok((
        input,
        NiniNode::Variable {
            name: name.to_string(),
            value,
        },
    ))
}

// Parser para una Clase
pub fn parse_class(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = tag("class ")(input)?;
    let (input, name) = alpha1(input)?;
    let (input, _) = line_ending(input)?;

    // Aquí buscamos funciones o elementos HTML que estén indentados debajo de la clase
    let (input, members) = many0(alt((parse_function, parse_html_element)))(input)?;

    Ok((
        input,
        NiniNode::Class {
            name: name.to_string(),
            members,
        },
    ))
}

// Parser para imports: import "./Component.nini" as ComponentName
fn parse_import(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = tag("import ")(input)?;
    let (input, _) = multispace0(input)?;

    // Parsear la ruta entre comillas
    let (input, _) = tag("\"")(input)?;
    let (input, path) = take_while(|c: char| c != '"')(input)?;
    let (input, _) = tag("\"")(input)?;

    let (input, _) = multispace0(input)?;
    let (input, _) = tag("as ")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, alias) = alpha1(input)?;
    let (input, _) = line_ending(input)?;

    Ok((
        input,
        NiniNode::Import {
            path: path.to_string(),
            alias: alias.to_string(),
        },
    ))
}

// Parser para un constructo (variable, clase o import)
fn parse_construct(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = multispace0(input)?;
    alt((parse_import, parse_variable, parse_class))(input)
}

// Parser para un archivo completo (formato antiguo)
pub fn parse_file(input: &str) -> IResult<&str, AbstractSyntaxTree> {
    let (input, _) = multispace0(input)?; // Ignorar espacios al inicio
    let (input, nodes) = many0(parse_construct)(input)?;
    let (input, _) = multispace0(input)?; // Ignorar espacios al final
    Ok((input, AbstractSyntaxTree { nodes }))
}

// Función que retorna solo los nodos (útil para codegen)
pub fn parse_nini_file(input: &str) -> IResult<&str, Vec<NiniNode>> {
    let (input, ast) = parse_file(input)?;
    Ok((input, ast.nodes))
}

// -------------------- NUEVA ESTRUCTURA DE COMPONENTE --------------------

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{alpha1, line_ending, multispace0, not_line_ending, space1},
    multi::many0,
    IResult,
};

// Estructura que representa un componente Nini
#[derive(Debug, Clone)]
pub struct Component {
    pub script: Vec<NiniNode>,
    pub template: String,
    pub style: String,
    pub file_path: String,
}

use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct ComponentResolver {
    pub components: HashMap<String, Component>,
    visited: Vec<String>,
}

impl ComponentResolver {
    pub fn new() -> Self {
        ComponentResolver {
            components: HashMap::new(),
            visited: Vec::new(),
        }
    }

    pub fn resolve_imports(
        &mut self,
        component: &mut Component,
        base_dir: &str,
    ) -> Result<(), String> {
        for node in &component.script {
            if let NiniNode::Import { path, alias } = node {
                let full_path = resolve_path(base_dir, path);

                if self.visited.contains(&full_path) {
                    return Err(format!("Dependencia cíclica detectada: {}", path));
                }

                self.visited.push(full_path.clone());

                match std::fs::read_to_string(&full_path) {
                    Ok(content) => {
                        let imported_path = Path::new(&full_path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| base_dir.to_string());

                        match parse_component_with_path(&content, full_path.clone()) {
                            Ok((_, mut imported_comp)) => {
                                self.resolve_imports(&mut imported_comp, &imported_path)?;
                                self.components.insert(alias.clone(), imported_comp);
                            }
                            Err(e) => {
                                return Err(format!("Error parseando {}: {:?}", path, e));
                            }
                        }
                    }
                    Err(e) => {
                        return Err(format!("No se pudo leer {}: {}", path, e));
                    }
                }
            }
        }
        Ok(())
    }
}

fn resolve_path(base_dir: &str, relative_path: &str) -> String {
    if relative_path.starts_with('/') || relative_path.contains(':') {
        return relative_path.to_string();
    }
    let clean_base = base_dir.trim_end_matches('/');
    format!("{}/{}", clean_base, relative_path)
}

pub fn parse_component_with_path(input: &str, file_path: String) -> IResult<&str, Component> {
    // Buscar el bloque <script>...</script>
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("<script>")(input)?;
    let (input, script_content) = take_while(|c| c != '<')(input)?;
    let (input, _) = tag("</script>")(input)?;

    let (_, script_ast) = parse_file(script_content)
        .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))?;

    let (input, template) = take_while(|_| true)(input)?;

    let (template, style) = if let Some(style_start) = template.find("<style>") {
        let (before, after) = template.split_at(style_start);
        let (style_content, _) = after.split_at(after.find("</style>").unwrap_or(after.len()));
        let style_content = &style_content[7..];
        (before.trim(), style_content.trim())
    } else {
        (template.trim(), "")
    };

    Ok((
        input,
        Component {
            script: script_ast.nodes,
            template: template.to_string(),
            style: style.to_string(),
            file_path,
        },
    ))
}

#[derive(Debug)]
struct StyleRule {
    selector: String,
    properties: Vec<(String, String)>,
}

// Parser que separa el archivo en script, template y style
pub fn parse_component(input: &str) -> IResult<&str, Component> {
    parse_component_with_path(input, "".to_string())
}

// Parser de estilo Nini (sintaxis con indentación y dos puntos)
fn parse_style(input: &str) -> Vec<StyleRule> {
    // Determinar la indentación común
    let mut common_indent = None;
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        match common_indent {
            None => common_indent = Some(indent),
            Some(existing) if indent < existing => common_indent = Some(indent),
            _ => {}
        }
    }
    let common_indent = common_indent.unwrap_or(0);

    let mut rules = Vec::new();
    let mut lines = input.lines().peekable();

    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        // Remover la indentación común
        let dedented = if line.len() > common_indent {
            &line[common_indent..]
        } else {
            line
        };
        let trimmed = dedented.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Si la línea termina con ':', es un selector
        if trimmed.ends_with(':') {
            let selector = trimmed.trim_end_matches(':').trim().to_string();
            let mut properties = Vec::new();
            // Leer líneas siguientes que están más indentadas que la indentación común
            while let Some(&next_line) = lines.peek() {
                if next_line.trim().is_empty() {
                    lines.next();
                    continue;
                }
                let next_indent = next_line.len() - next_line.trim_start().len();
                if next_indent > common_indent {
                    // Es una propiedad (indentada) o un sub-selector
                    let next_dedented = &next_line[next_indent - common_indent..];
                    let next_trimmed = next_dedented.trim();
                    if next_trimmed.ends_with(':') {
                        // Es un sub-selector, terminamos la regla actual
                        break;
                    }
                    if let Some(colon_pos) = next_trimmed.find(':') {
                        let prop = next_trimmed[..colon_pos].trim().to_string();
                        let value = next_trimmed[colon_pos + 1..].trim().to_string();
                        properties.push((prop, value));
                    }
                    lines.next();
                } else {
                    // No está más indentada, terminamos la regla actual
                    break;
                }
            }
            rules.push(StyleRule {
                selector,
                properties,
            });
        }
    }
    rules
}

// Generar CSS scoped a partir de reglas y un ID de componente
fn generate_scoped_css(rules: &[StyleRule], scope_class: &str) -> String {
    // HTML tags that should not be prefixed with a dot
    let html_tags = [
        "a",
        "abbr",
        "address",
        "area",
        "article",
        "aside",
        "audio",
        "b",
        "base",
        "bdi",
        "bdo",
        "blockquote",
        "body",
        "br",
        "button",
        "canvas",
        "caption",
        "cite",
        "code",
        "col",
        "colgroup",
        "data",
        "datalist",
        "dd",
        "del",
        "details",
        "dfn",
        "dialog",
        "div",
        "dl",
        "dt",
        "em",
        "embed",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "head",
        "header",
        "hgroup",
        "hr",
        "html",
        "i",
        "iframe",
        "img",
        "input",
        "ins",
        "kbd",
        "label",
        "legend",
        "li",
        "link",
        "main",
        "map",
        "mark",
        "menu",
        "meta",
        "meter",
        "nav",
        "noscript",
        "object",
        "ol",
        "optgroup",
        "option",
        "output",
        "p",
        "param",
        "picture",
        "pre",
        "progress",
        "q",
        "rp",
        "rt",
        "ruby",
        "s",
        "samp",
        "script",
        "section",
        "select",
        "small",
        "source",
        "span",
        "strong",
        "style",
        "sub",
        "summary",
        "sup",
        "table",
        "tbody",
        "td",
        "template",
        "textarea",
        "tfoot",
        "th",
        "thead",
        "time",
        "title",
        "tr",
        "track",
        "u",
        "ul",
        "var",
        "video",
        "wbr",
    ];

    let mut css = String::new();
    for rule in rules {
        // Check if selector is an HTML tag
        let base_selector = rule.selector.split_whitespace().next().unwrap_or("");
        let is_html_tag = html_tags.contains(&base_selector.to_lowercase().as_str());

        // Si el selector no empieza con . o # y no es un tag HTML, tratarlo como clase
        let processed_selector = if rule.selector.starts_with('.') || rule.selector.starts_with('#')
        {
            rule.selector.clone()
        } else if is_html_tag {
            // Keep HTML tags as-is
            rule.selector.clone()
        } else {
            // Treat as class selector
            format!(".{}", rule.selector)
        };

        // Prefijar el selector con la clase de scope
        let scoped_selector = format!(".{} {}", scope_class, processed_selector);
        css.push_str(&format!("{} {{\n", scoped_selector));
        for (prop, value) in &rule.properties {
            css.push_str(&format!("  {}: {};\n", prop, value));
        }
        css.push_str("}\n");
    }
    css
}

// Generador de código JavaScript para componentes
pub fn generate_component_js(
    component: &Component,
    scope_class: &str,
    resolved_components: &HashMap<String, Component>,
) -> (String, String) {
    let mut js_code = String::new();

    // Extraer imports del script
    let imports: Vec<(String, String)> = component
        .script
        .iter()
        .filter_map(|node| {
            if let NiniNode::Import { path, alias } = node {
                Some((path.clone(), alias.clone()))
            } else {
                None
            }
        })
        .collect();

    // Importar el runtime de Nini
    js_code.push_str("import { nini } from './nini-runtime-web/core.js';\n\n");

    // NO generar vars de componentes aquí - se generan después del innerHTML

    // Generar código para el script (variables, clases, etc.)
    for node in &component.script {
        match node {
            NiniNode::Variable { name, value } => {
                let js_value = if value.parse::<f64>().is_ok() {
                    value.to_string()
                } else {
                    format!("\"{}\"", value)
                };
                js_code.push_str(&format!("const {} = nini.signal({});\n", name, js_value));
            }
            NiniNode::Class { name, members } => {
                js_code.push_str(&format!("class {} {{\n", name));
                for member in members {
                    if let NiniNode::Function { name, body } = member {
                        js_code.push_str(&format!("  {}() {{\n", name));
                        if body.is_empty() {
                            js_code.push_str("    console.log(' ejecutar ');\n");
                        } else {
                            for expr_node in body {
                                if let NiniNode::Expression(expr) = expr_node {
                                    js_code.push_str(&format!("    {};\n", expr));
                                }
                            }
                        }
                        js_code.push_str("  }\n");
                    }
                }
                js_code.push_str("}\n");
            }
            NiniNode::Import { .. } => {}
            _ => {}
        }
    }

    // Resolver componentes en template
    let resolved_template =
        resolve_components_in_template(&component.template, &imports, resolved_components);

    // Generar el template HTML
    let template_html = transform_template(&resolved_template, scope_class);
    js_code.push_str(&format!(
        "document.getElementById('nini-app').innerHTML = `{}`;\n\n",
        template_html
    ));

    // NO pre-generar vars de componentes - se leen directamente en los efectos

    // Obtener los nombres de variables del app
    let app_var_names: Vec<String> = component
        .script
        .iter()
        .filter_map(|node| {
            if let NiniNode::Variable { name, .. } = node {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    // Crear efectos para las expresiones
    let parts = parse_template(&resolved_template);
    let mut effect_counter = 0;

    for part in parts {
        if let TemplatePart::Expression(expr) = part {
            effect_counter += 1;
            let span_id = format!("nini-expr-{}", effect_counter);

            // Si es variable del app, usar signal. Si no, leer del DOM
            if app_var_names.contains(&expr.to_string()) {
                js_code.push_str(&format!(
                    "nini.effect(() => {{\n    const el = document.getElementById('{}');\n    if (el) el.textContent = {}.value;\n}});\n",
                    span_id, expr
                ));
            } else {
                // Es variable de componente - buscar default en los componentes importados
                let default_val =
                    get_default_value_from_component(&expr, &imports, resolved_components);
                js_code.push_str(&format!(
                    "nini.effect(() => {{\n    const el = document.getElementById('{}');\n    if (el) {{\n        const comp = el.closest('.comp[data-instance]');\n        const instance = comp ? comp.getAttribute('data-instance') : null;\n        const propName = '{}';\n        const attr = instance ? `data-${{propName}}-${{instance}}` : null;\n        el.textContent = (attr && comp && comp.hasAttribute(attr)) ? comp.getAttribute(attr) : {};\n    }}\n}});\n",
                    span_id, expr, default_val
                ));
            }
        }
    }

    // Manejar eventos on_click (simplificado)
    if let Some(pos) = component.template.find("on_click=\"") {
        let start = pos + 10;
        if let Some(end) = component.template[start..].find('"') {
            let func = &component.template[start..start + end];
            if let Some(dot) = func.find('.') {
                let class_name = &func[..dot];
                let method_name = &func[dot + 1..];
                js_code.push_str(&format!(
                    "document.querySelector('button').addEventListener('click', () => {{\n    new {}().{}();\n}});\n",
                    class_name, method_name
                ));
            }
        }
    }

    // Generar CSS scoped (incluir CSS de componentes hijos)
    let mut all_css = generate_scoped_css(&parse_style(&component.style), scope_class);

    // Generate CSS for each imported component using alias only (no counter)
    for (_path, alias) in &imports {
        if let Some(comp) = resolved_components.get(alias) {
            all_css.push_str(&generate_scoped_css(
                &parse_style(&comp.style),
                alias, // Use just the alias (e.g., "Layout", "Card")
            ));
        }
    }

    (js_code, all_css)
}

// Genera variables para un componente importado
// Genera vars simples para componentes (sin sufijos)
fn get_default_value(expr: &str) -> String {
    if expr.parse::<f64>().is_ok() {
        expr.to_string()
    } else {
        format!("\"{}\"", expr)
    }
}

fn get_default_value_from_component(
    expr: &str,
    imports: &[(String, String)],
    components: &HashMap<String, Component>,
) -> String {
    // Buscar el default en los componentes importados
    for (_path, alias) in imports {
        if let Some(comp) = components.get(alias) {
            for node in &comp.script {
                if let NiniNode::Variable { name, value } = node {
                    if name == expr {
                        return format!("\"{}\"", value);
                    }
                }
            }
        }
    }
    // Default si no se encuentra
    format!("\"{}\"", expr)
}

fn generate_component_vars_simple(component: &Component) -> String {
    let mut js_code = String::new();

    for node in &component.script {
        match node {
            NiniNode::Variable { name, value } => {
                let default_value = if value.parse::<f64>().is_ok() {
                    value.to_string()
                } else {
                    format!("\"{}\"", value)
                };
                js_code.push_str(&format!(
                    "const {} = nini.signal(nini.prop('{}', {}));\n",
                    name, name, default_value
                ));
            }
            NiniNode::Class { name, members } => {
                js_code.push_str(&format!("class {} {{\n", name));
                for member in members {
                    if let NiniNode::Function {
                        name: fn_name,
                        body,
                    } = member
                    {
                        js_code.push_str(&format!("  {}() {{\n", fn_name));
                        if body.is_empty() {
                            js_code.push_str("    console.log(' ejecutar ');\n");
                        } else {
                            for expr_node in body {
                                if let NiniNode::Expression(expr) = expr_node {
                                    js_code.push_str(&format!("    {};\n", expr));
                                }
                            }
                        }
                        js_code.push_str("  }\n");
                    }
                }
                js_code.push_str("}\n");
            }
            _ => {}
        }
    }

    js_code
}

fn generate_component_vars(
    component: &Component,
    scope_class: &str,
    resolved_components: &HashMap<String, Component>,
    processed: &mut HashSet<String>,
) -> String {
    let mut js_code = String::new();

    // Procesar imports anidados
    let nested_imports: Vec<(String, String)> = component
        .script
        .iter()
        .filter_map(|node| {
            if let NiniNode::Import { path, alias } = node {
                Some((path.clone(), alias.clone()))
            } else {
                None
            }
        })
        .collect();

    for (_path, alias) in nested_imports {
        if !processed.contains(&alias) {
            processed.insert(alias.clone());
            if let Some(comp) = resolved_components.get(&alias) {
                // Use alias as scope (matches CSS)
                js_code.push_str(&generate_component_vars(
                    comp,
                    &alias, // Just the alias, no counter
                    resolved_components,
                    processed,
                ));
            }
        }
    }

    // Generar variables - usar nini.prop() para leer del DOM
    for node in &component.script {
        match node {
            NiniNode::Variable { name, value } => {
                let default_value = if value.parse::<f64>().is_ok() {
                    value.to_string()
                } else {
                    format!("\"{}\"", value)
                };
                js_code.push_str(&format!(
                    "const {} = nini.signal(nini.prop('{}', {}));\n",
                    name, name, default_value
                ));
            }
            NiniNode::Class { name, members } => {
                js_code.push_str(&format!("class {} {{\n", name));
                for member in members {
                    if let NiniNode::Function { name, body } = member {
                        js_code.push_str(&format!("  {}() {{\n", name));
                        if body.is_empty() {
                            js_code.push_str("    console.log(' ejecutar ');\n");
                        } else {
                            for expr_node in body {
                                if let NiniNode::Expression(expr) = expr_node {
                                    js_code.push_str(&format!("    {};\n", expr));
                                }
                            }
                        }
                        js_code.push_str("  }\n");
                    }
                }
                js_code.push_str("}\n");
            }
            _ => {}
        }
    }

    js_code
}

// Extrae props de un tag como <Card titulo="valor" />
fn extract_props(tag_content: &str) -> Vec<(String, String)> {
    let mut props = Vec::new();
    let mut current = tag_content.trim();

    while !current.is_empty() {
        current = current.trim();
        if current.is_empty() || current.starts_with('/') || current.starts_with('>') {
            break;
        }

        // Buscar nombre de prop
        let mut name_end = 0;
        for (i, c) in current.char_indices() {
            if c == '=' || c.is_whitespace() || c == '>' || c == '/' {
                break;
            }
            name_end = i + 1;
        }

        if name_end == 0 {
            break;
        }

        let prop_name = &current[..name_end];
        current = &current[name_end..].trim();

        if current.starts_with('=') {
            current = &current[1..].trim();
            let (value, rest) = if current.starts_with('"') {
                let quote = current[1..]
                    .find('"')
                    .map(|p| p + 1)
                    .unwrap_or(current.len());
                (&current[1..quote], &current[quote + 1..])
            } else if current.starts_with('\'') {
                let quote = current[1..]
                    .find('\'')
                    .map(|p| p + 1)
                    .unwrap_or(current.len());
                (&current[1..quote], &current[quote + 1..])
            } else {
                let end = current
                    .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                    .unwrap_or(current.len());
                (&current[..end], &current[end..])
            };
            props.push((prop_name.to_string(), value.to_string()));
            current = rest;
        }
    }

    props
}

// Cuenta las instancias de cada componente
fn count_component_instances(template: &str, alias: &str) -> usize {
    let with_attrs = format!("<{} ", alias);
    let self_closing = format!("<{}/>", alias);
    let self_closing_space = format!("<{} />", alias);

    let mut count = 0;
    let mut search = template;

    while search.contains(&with_attrs)
        || search.contains(&self_closing)
        || search.contains(&self_closing_space)
    {
        if let Some(pos) = search.find(&with_attrs) {
            search = &search[pos + with_attrs.len()..];
            count += 1;
        } else if let Some(pos) = search.find(&self_closing) {
            search = &search[pos + self_closing.len()..];
            count += 1;
        } else if let Some(pos) = search.find(&self_closing_space) {
            search = &search[pos + self_closing_space.len()..];
            count += 1;
        } else {
            break;
        }
    }

    count
}

// Genera variables únicas para N instancias de un componente
fn generate_component_instance_vars(
    component: &Component,
    base_scope: &str,
    instance_index: usize,
) -> String {
    let mut js_code = String::new();
    let instance_suffix = format!("_{}", instance_index);

    for node in &component.script {
        match node {
            NiniNode::Variable { name, value } => {
                let default_value = if value.parse::<f64>().is_ok() {
                    value.to_string()
                } else {
                    format!("\"{}\"", value)
                };
                let var_name = format!("{}{}", name, instance_suffix);
                js_code.push_str(&format!(
                    "const {} = nini.signal(nini.prop('{}{}', {}));\n",
                    var_name, name, instance_suffix, default_value
                ));
            }
            NiniNode::Class { name, members } => {
                js_code.push_str(&format!("class {} {{\n", name));
                for member in members {
                    if let NiniNode::Function {
                        name: fn_name,
                        body,
                    } = member
                    {
                        js_code.push_str(&format!("  {}() {{\n", fn_name));
                        if body.is_empty() {
                            js_code.push_str("    console.log(' ejecutar ');\n");
                        } else {
                            for expr_node in body {
                                if let NiniNode::Expression(expr) = expr_node {
                                    js_code.push_str(&format!("    {};\n", expr));
                                }
                            }
                        }
                        js_code.push_str("  }\n");
                    }
                }
                js_code.push_str("}\n");
            }
            _ => {}
        }
    }

    js_code
}

// Reemplaza <Componente /> con el template del componente
fn resolve_components_in_template(
    template: &str,
    imports: &[(String, String)],
    components: &HashMap<String, Component>,
) -> String {
    let mut result = template.to_string();

    for (_path, alias) in imports {
        if let Some(comp) = components.get(alias) {
            // Replace with attributes: <Card titulo="..." />
            let with_attrs = format!("<{} ", alias);
            while result.contains(&with_attrs) {
                if let Some(pos) = result.find(&with_attrs) {
                    let rest = &result[pos + with_attrs.len()..];
                    if let Some(close_pos) = rest.find('>') {
                        let tag_content = &rest[..close_pos];
                        let props = extract_props(tag_content);

                        // Generate unique ID for this instance
                        let instance_id = format!(
                            "{}_{}",
                            alias,
                            result.matches(&with_attrs).count()
                                + result.matches(&format!("<{}/>", alias)).count()
                        );

                        // Convert props to data attributes with instance suffix
                        let props_attrs: String = props
                            .iter()
                            .map(|(k, v)| {
                                format!("data-{}-{}=\"{}\"", k.to_lowercase(), instance_id, v)
                            })
                            .collect::<Vec<_>>()
                            .join(" ");

                        let replacement = format!(
                            "<div class=\"comp {alias}\" data-instance=\"{}\" {}>{}</div>",
                            instance_id, props_attrs, comp.template
                        );

                        let end = pos + with_attrs.len() + close_pos + 1;
                        result = format!("{}{}{}", &result[..pos], replacement, &result[end..]);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Replace self-closing: <Card />
            let self_closing = format!("<{}/>", alias);
            let mut instance_counter = 0;
            while result.contains(&self_closing) {
                if let Some(pos) = result.find(&self_closing) {
                    instance_counter += 1;
                    let instance_id = format!("{}_{}", alias, instance_counter);

                    // Add alias class to the comp wrapper for CSS scoping
                    let replacement = format!(
                        "<div class=\"comp {alias}\" data-instance=\"{}\">{}</div>",
                        instance_id, comp.template
                    );
                    let end = pos + self_closing.len();
                    result = format!("{}{}{}", &result[..pos], replacement, &result[end..]);
                }
            }

            // Replace with children: <Layout>...content...</Layout>
            let with_children_start = format!("<{}>", alias);
            let with_children_end = format!("</{}>", alias);
            let mut children_counter: usize = 0;

            while let Some(start_pos) = result.find(&with_children_start) {
                if let Some(end_pos) = result[start_pos..].find(&with_children_end) {
                    children_counter += 1;
                    let content_start = start_pos + with_children_start.len();
                    let content_end = start_pos + end_pos;
                    let children = &result[content_start..content_end];
                    let instance_id = format!("{}_{}", alias, children_counter);

                    // First resolve any components in the children
                    let children_resolved =
                        resolve_components_in_template(children, imports, components);

                    // Replace slot directly with children content
                    let template_with_children = comp
                        .template
                        .replace("<slot />", &children_resolved)
                        .replace("<slot></slot>", &children_resolved);

                    // Add alias class to the comp wrapper for CSS scoping
                    let replacement = format!(
                        "<div class=\"comp {alias}\" data-instance=\"{}\">{}</div>",
                        instance_id, template_with_children
                    );

                    let full_end = content_end + with_children_end.len();
                    result = format!(
                        "{}{}{}",
                        &result[..start_pos],
                        replacement,
                        &result[full_end..]
                    );
                } else {
                    break;
                }
            }
        }
    }

    result
}

// Transforma el template HTML reemplazando {expr} con spans con IDs únicas.
fn transform_template(template: &str, scope_class: &str) -> String {
    // First transform Link components
    let mut result = template.to_string();

    // Transform <Link to="...">text</Link> to <a href="..." data-nini-link>text</a>
    while let Some(start) = result.find("<Link ") {
        if let Some(to_pos) = result[start..].find("to=\"") {
            let path_start = start + to_pos + 4;
            if let Some(path_end) = result[path_start..].find('"') {
                let path = &result[path_start..path_start + path_end];
                let after_path = path_start + path_end;
                if let Some(tag_end) = result[after_path..].find('>') {
                    let content_start = after_path + tag_end + 1;
                    if let Some(close_pos) = result[content_start..].find("</Link>") {
                        let content = &result[content_start..content_start + close_pos];
                        let replacement =
                            format!(r#"<a href="{}" data-nini-link>{}</a>"#, path, content);
                        let full_end = content_start + close_pos + 7;
                        result =
                            format!("{}{}{}", &result[..start], replacement, &result[full_end..]);
                        continue;
                    }
                }
            }
        }
        break;
    }

    // Now transform expressions {expr} -> <span>
    let mut result_final = String::new();
    let mut current = result.as_str();
    let mut expr_counter = 0;

    while !current.is_empty() {
        if let Some(start) = current.find('{') {
            if start > 0 {
                result_final.push_str(&current[..start]);
            }
            if let Some(end) = current[start..].find('}') {
                expr_counter += 1;
                let expr = &current[start + 1..start + end];
                let span = format!(
                    r#"<span id="nini-expr-{}">{{{}.value}}</span>"#,
                    expr_counter,
                    expr.trim()
                );
                result_final.push_str(&span);
                current = &current[start + end + 1..];
            } else {
                result_final.push_str(&current[start..]);
                break;
            }
        } else {
            result_final.push_str(current);
            break;
        }
    }
    format!(r#"<div class="{}">{}</div>"#, scope_class, result_final)
}

// Generador de código JavaScript (legacy, para compatibilidad)
pub fn generate_js(nodes: &[NiniNode]) -> String {
    // Esta función ya no se usará para el nuevo formato, pero la mantenemos por compatibilidad.
    let mut js_code = String::new();

    // Importar el runtime de Nini
    js_code.push_str("import { nini } from './nini-runtime-web/core.js';\n\n");

    // Generar código para el script (variables, clases, etc.)
    for node in nodes {
        match node {
            NiniNode::Variable { name, value } => {
                let js_value = if value.parse::<f64>().is_ok() {
                    value.to_string()
                } else {
                    format!("\"{}\"", value)
                };
                js_code.push_str(&format!("const {} = nini.signal({});\n", name, js_value));
            }
            NiniNode::Class { name, members } => {
                js_code.push_str(&format!("class {} {{\n", name));
                for member in members {
                    match member {
                        NiniNode::Function { name, body } => {
                            js_code.push_str(&format!("  {}() {{\n", name));
                            if body.is_empty() {
                                js_code.push_str(&format!(
                                    "    console.log('Ejecutando {}');\n",
                                    name
                                ));
                            } else {
                                for expr_node in body {
                                    if let NiniNode::Expression(expr) = expr_node {
                                        js_code.push_str(&format!("    {};\n", expr));
                                    }
                                }
                            }
                            js_code.push_str("  }\n");
                        }
                        NiniNode::HtmlElement {
                            tag,
                            attributes,
                            content,
                        } => {
                            // Ignorar elementos HTML en el generate_js legacy
                        }
                        _ => {}
                    }
                }
                js_code.push_str("}\n");
            }
            _ => {}
        }
    }

    // ... (resto del generador anterior)

    js_code
}

// El resto de tests...

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_multiple_classes() {
        let input = "class Usuario\n  fn saludar()\n\nclass Producto\n  fn describir()\n";
        let result = parse_file(input);
        assert!(result.is_ok());
        let (_, ast) = result.unwrap();
        assert_eq!(ast.nodes.len(), 2);
        assert!(matches!(&ast.nodes[0], NiniNode::Class { name, .. } if name == "Usuario"));
        assert!(matches!(&ast.nodes[1], NiniNode::Class { name, .. } if name == "Producto"));
    }

    #[test]
    fn test_parse_variable_and_class() {
        let input = "nombre = \"Ricky\"\nclass Usuario\n  fn saludar()\n";
        let result = parse_file(input);
        assert!(result.is_ok());
        let (_, ast) = result.unwrap();
        assert_eq!(ast.nodes.len(), 2);
        assert!(matches!(&ast.nodes[0], NiniNode::Variable { name, .. } if name == "nombre"));
        assert!(matches!(&ast.nodes[1], NiniNode::Class { .. }));
    }

    #[test]
    fn test_slot_replaced_with_children() {
        // Test that <slot /> is replaced with children content
        let component_template = "<div class=\"layout\"><slot /></div>";
        let children = "<p>Hello World</p>";

        let result = component_template
            .replace("<slot />", children)
            .replace("<slot></slot>", children);

        assert_eq!(result, "<div class=\"layout\"><p>Hello World</p></div>");
        assert!(!result.contains("<slot"));
    }

    #[test]
    fn test_slot_with_nested_html_children() {
        // Test slot with nested HTML
        let component_template = "<div class=\"container\"><main><slot /></main></div>";
        let children = "<div class=\"content\"><h1>Title</h1><p>Paragraph</p></div>";

        let result = component_template
            .replace("<slot />", children)
            .replace("<slot></slot>", children);

        assert_eq!(result, "<div class=\"container\"><main><div class=\"content\"><h1>Title</h1><p>Paragraph</p></div></main></div>");
        assert!(!result.contains("<slot"));
    }

    #[test]
    fn test_slot_self_closing_vs_block_syntax() {
        // Test both <slot /> and <slot></slot>
        let template1 = "<div><slot /></div>";
        let template2 = "<div><slot></slot></div>";
        let children = "<span>Child</span>";

        let result1 = template1
            .replace("<slot />", children)
            .replace("<slot></slot>", children);

        let result2 = template2
            .replace("<slot />", children)
            .replace("<slot></slot>", children);

        assert_eq!(result1, "<div><span>Child</span></div>");
        assert_eq!(result2, "<div><span>Child</span></div>");
    }

    #[test]
    fn test_component_with_children_generates_correct_html() {
        let template = r#"<script>
    prop titulo = "Default"
</script>

<div class="card">
    <h2>{titulo}</h2>
    <slot />
</div>

<style>
    card:
        background: #333
        padding: 1rem
    h2:
        color: gold
</style>"#;

        let result = parse_component(template);
        assert!(result.is_ok());
        let (_, component) = result.unwrap();

        // Verify template contains <slot />
        assert!(
            component.template.contains("<slot />") || component.template.contains("<slot></slot>")
        );

        // Test slot replacement
        let children = "<p>Card content</p>";
        let rendered = component
            .template
            .replace("<slot />", children)
            .replace("<slot></slot>", children);

        assert!(rendered.contains("<p>Card content</p>"));
        assert!(!rendered.contains("<slot"));
    }

    #[test]
    fn test_nested_component_slot_resolution() {
        // Test resolving components that have slots
        let mut components = std::collections::HashMap::new();

        components.insert(
            "Layout".to_string(),
            Component {
                template: "<div class=\"layout\"><nav>Nav</nav><main><slot /></main></div>"
                    .to_string(),
                script: vec![],
                style: "".to_string(),
                file_path: "Layout.nini".to_string(),
            },
        );

        let template = "<Layout><p>Page content</p></Layout>";
        // Need to provide imports for the component to be resolved
        let imports = vec![("components/Layout.nini".to_string(), "Layout".to_string())];

        let result = resolve_components_in_template(template, &imports, &components);

        // Debug output
        println!("Result: {}", result);

        // Should not contain <Layout> tags
        assert!(
            !result.contains("<Layout>"),
            "Result should not contain <Layout>, but got: {}",
            result
        );
        assert!(
            !result.contains("</Layout>"),
            "Result should not contain </Layout>"
        );

        // Should contain the slot content
        assert!(
            result.contains("<p>Page content</p>"),
            "Result should contain slot content"
        );

        // Should contain the layout structure (layout class is present)
        assert!(
            result.contains("layout"),
            "Result should contain layout class"
        );
        assert!(
            result.contains("<nav>Nav</nav>"),
            "Result should contain nav"
        );
        assert!(
            result.contains("<slot />") == false,
            "Result should not contain <slot />"
        );
    }
}
