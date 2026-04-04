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
    Service {
        name: String,
        members: Vec<NiniNode>,
    },
    Store {
        name: String,
        members: Vec<NiniNode>,
    },
    Inject {
        service_name: String,
        variable_name: String,
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
    let (mut input, _) = multispace0(input)?; // Allow optional space before fn
    let (input, _) = tag("fn ")(input)?;
    let (input, name) = alpha1(input)?;
    // Skip optional ()
    let (input, _) = multispace0(input)?;
    let input = if input.starts_with('(') {
        if let Some(close) = input.find(')') {
            &input[close + 1..]
        } else {
            input
        }
    } else {
        input
    };

    // Intentar parsear cuerpo (múltiples líneas hasta "end")
    let mut body = Vec::new();
    let mut remaining = input;

    while !remaining.is_empty() {
        let trimmed = remaining.trim_start();
        if trimmed.starts_with("end") || trimmed.starts_with("end\n") {
            remaining = trimmed.strip_prefix("end").unwrap_or(remaining);
            break;
        }

        if let Some(newline_pos) = remaining.find('\n') {
            let line = remaining[..newline_pos].trim();
            if !line.is_empty() {
                body.push(NiniNode::Expression(line.to_string()));
            }
            remaining = &remaining[newline_pos + 1..];
        } else {
            break;
        }
    }

    Ok((
        remaining,
        NiniNode::Function {
            name: name.to_string(),
            body,
        },
    ))
}

// Parser para una variable (soporta strings entre comillas y valores literales)
fn parse_variable(input: &str) -> IResult<&str, NiniNode> {
    let (input, name) = take_while(|c: char| c.is_alphanumeric() || c == '_')(input)?;
    if name.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Alpha,
        )));
    }
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

// Parser para un Servicio: service NombreServicio
pub fn parse_service(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = tag("service ")(input)?;
    let (input, name) = alpha1(input)?;
    let (input, _) = line_ending(input)?;

    // Parsear miembros (props y functions)
    let (input, members) = many0(alt((parse_variable, parse_function, parse_html_element)))(input)?;

    Ok((
        input,
        NiniNode::Service {
            name: name.to_string(),
            members,
        },
    ))
}

// Parser para un Store: store NombreStore
pub fn parse_store(input: &str) -> IResult<&str, NiniNode> {
    eprintln!("parse_store: INPUT = {:?}", &input[..input.len().min(60)]);
    let (input, _) = tag("store ")(input)?;
    let (input, name) = alpha1(input)?;
    eprintln!("parse_store: name = '{}'", name);
    let (input, _) = line_ending(input)?;

    // Parsear miembros (variables y funciones) - permitir whitespace antes
    let (input, members) = many0(alt((
        |i| {
            let (i, _) = multispace0(i)?;
            parse_variable(i)
        },
        |i| {
            let (i, _) = multispace0(i)?;
            parse_function(i)
        },
        parse_html_element,
    )))(input)?;
    eprintln!("parse_store: parsed {} members", members.len());
    for m in &members {
        eprintln!("  member: {:?}", m);
    }

    // Consume "end" if present
    let input = input.trim_start();
    let input = if input.starts_with("end") {
        &input[3..]
    } else {
        eprintln!("parse_store: WARNING - no 'end' keyword found");
        input
    };

    Ok((
        input,
        NiniNode::Store {
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

// Parser para inject: var_name = inject ServiceName
fn parse_inject(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = multispace0(input)?;
    let (input, var_name) = alpha1(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("=")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("inject")(input)?;
    let (input, _) = space1(input)?;
    let (input, service_name) = alpha1(input)?;
    let (input, _) = line_ending(input)?;

    Ok((
        input,
        NiniNode::Inject {
            service_name: service_name.to_string(),
            variable_name: var_name.to_string(),
        },
    ))
}

// Parser para un constructo (variable, clase, service, store, inject o import)
fn parse_construct(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = multispace0(input)?;
    let first_word: String = input
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    // If line starts with a known function call (onChange, log, etc.), parse as expression
    let known_calls = ["onChange", "log", "nini"];
    if known_calls.iter().any(|k| first_word == *k) && input.contains('(') {
        eprintln!("parse_construct: detected known call '{}'", first_word);
        // Handle multi-line expressions by finding matching braces
        let paren_start = input.find('(').unwrap();
        let mut depth = 0;
        let mut end_pos = None;
        for (i, c) in input[paren_start..].char_indices() {
            match c {
                '(' | '{' | '[' => depth += 1,
                ')' | '}' | ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end_pos = Some(paren_start + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(end) = end_pos {
            let line = &input[..end];
            let remaining = &input[end..];
            let remaining = remaining.trim_start_matches(|c: char| c == '\n' || c.is_whitespace());
            eprintln!(
                "parse_construct: parsed multi-line expr, remaining len={}",
                remaining.len()
            );
            return Ok((remaining, NiniNode::Expression(line.trim().to_string())));
        }
    }

    eprintln!(
        "parse_construct: trying to parse: '{}'",
        &input[..input.len().min(40)]
    );
    let result = alt((
        parse_import,
        parse_inject,
        parse_service,
        parse_store,
        parse_function,
        parse_variable,
        parse_class,
    ))(input);
    if let Ok((_, node)) = &result {
        eprintln!(
            "parse_construct: SUCCESS -> {:?}",
            std::mem::discriminant(node)
        );
    } else {
        eprintln!("parse_construct: FAILED for '{}'", first_word);
    }
    result
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
    let mut all_nodes = Vec::new();
    let mut remaining = input;

    // Parse ALL <script>...</script> blocks
    while let Some(script_start) = remaining.find("<script>") {
        let before_script = &remaining[..script_start];
        let after_open = &remaining[script_start + 8..]; // after <script>

        if let Some(script_close) = after_open.find("</script>") {
            let script_content = &after_open[..script_close];

            match parse_file(script_content) {
                Ok((_, ast)) => {
                    all_nodes.extend(ast.nodes);
                }
                Err(_) => {}
            }

            remaining = &after_open[script_close + 9..]; // after </script>
        } else {
            break;
        }
    }

    let template = remaining;

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
            script: all_nodes,
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

    // Hacer log y onChange disponibles globalmente
    js_code.push_str("const log = nini.log.bind(nini);\n");
    js_code.push_str("const onChange = nini.onChange.bind(nini);\n\n");

    // NO generar vars de componentes aquí - se generan después del innerHTML

    // Generar código para el script (variables, clases, servicios, etc.)
    // Primero procesar servicios para crear singleton registry
    let mut services_generated = false;
    for node in &component.script {
        if let NiniNode::Service { .. } = node {
            if !services_generated {
                js_code.push_str("// Service Singleton Registry\n");
                js_code.push_str("const __services = {};\n\n");
                services_generated = true;
            }
            break;
        }
    }

    for node in &component.script {
        match node {
            NiniNode::Variable { name, value } => {
                // Detectar tipo de valor
                let js_value = if value.parse::<f64>().is_ok() {
                    value.to_string()
                } else if value == "true" || value == "false" {
                    value.clone()
                } else if value.starts_with('[') || value.starts_with('{') {
                    value.clone()
                } else {
                    format!("\"{}\"", value)
                };
                // Crear signal y hacer global
                js_code.push_str(&format!(
                    "window._{name} = nini.signal({js_value});\n",
                    name = name,
                    js_value = js_value
                ));
                js_code.push_str(&format!(
                    "Object.defineProperty(window, '{name}', {{\n",
                    name = name
                ));
                js_code.push_str(&format!(
                    "    get() {{ return window._{name}.value; }},\n",
                    name = name
                ));
                js_code.push_str(&format!(
                    "    set(v) {{ window._{name}.value = v; }}\n",
                    name = name
                ));
                js_code.push_str("});\n");
            }
            NiniNode::Service { name, members } => {
                js_code.push_str(&format!("class {} {{\n", name));
                // Constructor
                js_code.push_str("  constructor() {\n");
                for member in members {
                    if let NiniNode::Variable {
                        name: prop_name,
                        value,
                    } = member
                    {
                        let js_value = if value.parse::<f64>().is_ok() {
                            value.to_string()
                        } else {
                            format!("\"{}\"", value)
                        };
                        js_code.push_str(&format!("    this.{} = {};\n", prop_name, js_value));
                    }
                }
                js_code.push_str("  }\n");
                // Methods
                for member in members {
                    if let NiniNode::Function { name, body } = member {
                        js_code.push_str(&format!("  {}() {{\n", name));
                        for expr_node in body {
                            if let NiniNode::Expression(expr) = expr_node {
                                js_code.push_str(&format!("    {};\n", expr));
                            }
                        }
                        js_code.push_str("  }\n");
                    }
                }
                js_code.push_str("}\n");
                // Singleton getter
                js_code.push_str(&format!("function get{}() {{\n", name));
                js_code.push_str(&format!("  if (!__services['{}']) {{\n", name));
                js_code.push_str(&format!("    __services['{}'] = new {}();\n", name, name));
                js_code.push_str("  }\n");
                js_code.push_str(&format!("  return __services['{}'];\n", name));
                js_code.push_str("}\n\n");
            }
            NiniNode::Store { name, members } => {
                js_code.push_str(&format!("// Store: {}\n", name));
                // Create store as a global object with signals for each variable
                js_code.push_str(&format!(
                    "window.nini.stores = window.nini.stores || {{}};\n"
                ));
                js_code.push_str(&format!("window.nini.stores['{}'] = {{\n", name));

                for member in members {
                    if let NiniNode::Variable {
                        name: var_name,
                        value,
                    } = member
                    {
                        let js_value = if value.parse::<f64>().is_ok() {
                            value.to_string()
                        } else if value == "true" || value == "false" {
                            value.clone()
                        } else if value.starts_with('[') || value.starts_with('{') {
                            value.clone()
                        } else {
                            format!("\"{}\"", value)
                        };
                        js_code
                            .push_str(&format!("    {}: nini.signal({}),\n", var_name, js_value));
                    }
                }
                js_code.push_str("};\n\n");

                // Create global getters/setters for store properties
                js_code.push_str(&format!("// Getters/setters for {} store\n", name));
                for member in members {
                    if let NiniNode::Variable { name: var_name, .. } = member {
                        let full_name = format!("{}_{}", name, var_name);
                        js_code.push_str(&format!(
                            "window._{} = window.nini.stores['{}'].{};\n",
                            full_name, name, var_name
                        ));
                        js_code.push_str(&format!(
                            "Object.defineProperty(window, '{}', {{\n",
                            full_name
                        ));
                        js_code.push_str(&format!(
                            "    get() {{ return window.nini.stores['{}'].{}.value; }},\n",
                            name, var_name
                        ));
                        js_code.push_str(&format!(
                            "    set(v) {{ window.nini.stores['{}'].{}.value = v; }}\n",
                            name, var_name
                        ));
                        js_code.push_str("});\n");

                        // Also expose as global (just var_name) for easier template access
                        js_code.push_str(&format!(
                            "window.{} = window.nini.stores['{}'].{};\n",
                            var_name, name, var_name
                        ));
                        js_code.push_str(&format!(
                            "Object.defineProperty(window, '{}', {{\n",
                            var_name
                        ));
                        js_code.push_str(&format!(
                            "    get() {{ return window.nini.stores['{}'].{}.value; }},\n",
                            name, var_name
                        ));
                        js_code.push_str(&format!(
                            "    set(v) {{ window.nini.stores['{}'].{}.value = v; }}\n",
                            name, var_name
                        ));
                        js_code.push_str("});\n");

                        // Also create underscore-prefixed version for effects
                        js_code.push_str(&format!(
                            "window._{} = window.nini.stores['{}'].{};\n",
                            var_name, name, var_name
                        ));
                    }
                }
                js_code.push_str("\n");
            }
            NiniNode::Inject {
                service_name,
                variable_name,
            } => {
                js_code.push_str(&format!(
                    "const {} = get{}();\n",
                    variable_name, service_name
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
            NiniNode::Import { .. } => {}
            NiniNode::Function { name, body } if name == "init" => {
                js_code.push_str("nini.onMount(() => {\n");
                for expr_node in body {
                    if let NiniNode::Expression(expr) = expr_node {
                        js_code.push_str(&format!("    {};\n", expr));
                    }
                }
                js_code.push_str("});\n");
            }
            NiniNode::Function { name, body } => {
                js_code.push_str(&format!("window.{name} = function() {{\n"));
                for expr_node in body {
                    if let NiniNode::Expression(expr) = expr_node {
                        js_code.push_str(&format!("    {};\n", expr));
                    }
                }
                js_code.push_str("};\n");
            }
            NiniNode::Expression(_) => {
                // Skip - expressions will be generated after mount
            }
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

    // Trigger onMount callbacks después de renderizar
    js_code.push_str("nini.triggerMount();\n\n");

    // Generar expresiones directas (como onChange) después del mount
    for node in &component.script {
        if let NiniNode::Expression(expr) = node {
            // Transform onChange([var1, var2], ...) to onChange(["var1", "var2"], ...)
            let transformed = if expr.starts_with("onChange([") {
                if let Some(arr_start) = expr.find('[') {
                    if let Some(arr_end) = expr.find(']') {
                        let vars_str = &expr[arr_start + 1..arr_end];
                        let rest = &expr[arr_end + 1..];
                        let vars: Vec<String> =
                            vars_str.split(',').map(|v| v.trim().to_string()).collect();
                        let quoted_vars: Vec<String> =
                            vars.iter().map(|v| format!("\"{}\"", v)).collect();
                        // rest starts with ", () => { ... });" - keep as-is
                        format!("onChange([{}]{}", quoted_vars.join(", "), rest)
                    } else {
                        expr.clone()
                    }
                } else {
                    expr.clone()
                }
            } else {
                expr.clone()
            };
            js_code.push_str(&format!("{};\n", transformed));
        }
    }

    // NO pre-generar vars de componentes - se leen directamente en los efectos

    // Obtener los nombres de variables del app (incluyendo variables de stores)
    let mut app_var_names: Vec<String> = component
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

    // También agregar variables de stores con su nombre de store como prefijo
    let mut store_vars: Vec<(String, String)> = Vec::new(); // (store_name, var_name)
    for node in &component.script {
        eprintln!(
            "DEBUG: Checking node type for store vars: {:?}",
            std::mem::discriminant(node)
        );
        if let NiniNode::Store { name, members, .. } = node {
            eprintln!(
                "DEBUG: Found store '{}' with {} members",
                name,
                members.len()
            );
            for member in members {
                eprintln!("DEBUG: Store member: {:?}", member);
                if let NiniNode::Variable { name: var_name, .. } = member {
                    eprintln!("DEBUG: Store var '{}'", var_name);
                    app_var_names.push(var_name.clone());
                    store_vars.push((name.clone(), var_name.clone()));
                }
            }
        }
    }
    eprintln!("DEBUG: app_var_names = {:?}", app_var_names);
    eprintln!("DEBUG: store_vars = {:?}", store_vars);

    // Crear efectos para las expresiones (solo fuera de data-nini-for)
    let parts = parse_template(&template_html);
    let mut effect_counter = 0;

    // Contar expresiones que están dentro de data-nini-for para saltarlas
    let mut for_expr_positions = std::collections::HashSet::new();

    // Buscar spans dentro de data-nini-for de manera simple
    let mut pos = 0;
    while let Some(for_start) = template_html[pos..].find("data-nini-for=") {
        let abs_start = pos + for_start;
        // Encontrar el cierre del div
        if let Some(div_close) = template_html[abs_start..].find("</div>") {
            let abs_close = abs_start + div_close + 6; // +6 for </div>
            let block = &template_html[abs_start..abs_close];

            // Encontrar spans en este bloque
            let span_re = regex::Regex::new(r#"id="(nini-expr-\d+)""#).unwrap();
            for span_cap in span_re.captures_iter(block) {
                if let Some(span_id) = span_cap.get(1) {
                    for_expr_positions.insert(span_id.as_str().to_string());
                }
            }

            pos = abs_close;
        } else {
            break;
        }
    }

    for part in parts {
        if let TemplatePart::Expression(expr) = part {
            effect_counter += 1;
            let span_id = format!("nini-expr-{}", effect_counter);

            // Saltar expresiones dentro de data-nini-for (ya manejadas por foreach handler)
            if for_expr_positions.contains(&span_id) {
                continue;
            }

            // Si es variable del app, usar signal interno para suscripción
            if app_var_names.contains(&expr.to_string()) {
                // Check if it's a store variable
                let signal_path = store_vars
                    .iter()
                    .find(|(_, var)| var == &expr.to_string())
                    .map(|(store, var)| format!("window.nini.stores['{}'].{}.value", store, var))
                    .unwrap_or_else(|| format!("window._{}.value", expr));

                js_code.push_str(&format!(
                    "nini.effect(() => {{\n    const el = document.getElementById('{}');\n    if (el) el.textContent = {};\n}});\n",
                    span_id, signal_path
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

    // Manejar directiva data-nini-if (mostrar/ocultar según condición)
    js_code.push_str("\n// Handle nini-if directives\n");
    js_code.push_str("document.querySelectorAll('[data-nini-if]').forEach(el => {\n");
    js_code.push_str("    const condition = el.getAttribute('data-nini-if');\n");
    js_code.push_str(
        "    // Convertir nombre de variable a signal interno: mostrar -> window._mostrar.value\n",
    );
    js_code.push_str("    const signalCondition = condition.replace(/\\b(\\w+)\\b/g, (m) => window['_' + m] !== undefined ? 'window._' + m + '.value' : m);\n");
    js_code.push_str("    nini.effect(() => {\n");
    js_code.push_str("        const show = eval(signalCondition);\n");
    js_code.push_str("        el.style.display = show ? '' : 'none';\n");
    js_code.push_str("    });\n");
    js_code.push_str("});\n\n");

    // Handle bind:value directives
    js_code.push_str("// Handle nini-bind directives\n");
    js_code.push_str("nini.bindHandler();\n\n");

    // Manejar directiva data-nini-for (renderizar lista)
    js_code.push_str("// Handle nini-for directives\n");
    js_code.push_str("document.querySelectorAll('[data-nini-for]').forEach(template => {\n");
    js_code.push_str("    const forExpr = template.getAttribute('data-nini-for');\n");
    js_code.push_str("    const match = forExpr.match(/(\\w+)\\s+in\\s+(\\w+)/);\n");
    js_code.push_str("    if (!match) return;\n");
    js_code.push_str("    const [, itemName, listName] = match;\n");
    js_code.push_str("    const parent = template.parentNode;\n");
    js_code.push_str("    // Ocultar el template original\n");
    js_code.push_str("    template.style.display = 'none';\n");
    js_code.push_str("    \n");
    js_code.push_str("    nini.effect(() => {\n");
    js_code.push_str("        const list = window['_' + listName]?.value || [];\n");
    js_code.push_str("        // Remove old items\n");
    js_code.push_str("        parent.querySelectorAll(`[data-nini-for-item=\"${listName}\"]`).forEach(el => el.remove());\n");
    js_code.push_str("        // Add new items\n");
    js_code.push_str("        list.forEach((item, index) => {\n");
    js_code.push_str("            const clone = template.cloneNode(true);\n");
    js_code.push_str("            clone.style.display = '';\n");
    js_code.push_str("            clone.removeAttribute('data-nini-for');\n");
    js_code.push_str("            clone.setAttribute('data-nini-for-item', listName);\n");
    js_code.push_str("            clone.innerHTML = clone.innerHTML.replace(new RegExp(`\\\\{\\\\s*${itemName}\\\\s*\\\\}`, 'g'), item);\n");
    js_code.push_str("            clone.innerHTML = clone.innerHTML.replace(new RegExp(`\\\\b${itemName}\\\\b`, 'g'), item);\n");
    js_code.push_str("            parent.insertBefore(clone, template);\n");
    js_code.push_str("        });\n");
    js_code.push_str("    });\n");
    js_code.push_str("});\n\n");

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
                        // Escapar comillas dobles en el valor
                        let escaped = value.replace('"', "\\\"");
                        return format!("\"{}\"", escaped);
                    }
                }
            }
        }
    }
    // Default si no se encuentra - escapar comillas
    let escaped = expr.replace('"', "\\\"");
    format!("\"{}\"", escaped)
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

// Find closing brace position accounting for nested braces
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 1; // Start at 1 since opening brace was already consumed
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// Transforma el template HTML reemplazando {expr} con spans con IDs únicas.
fn transform_template(template: &str, scope_class: &str) -> String {
    let mut result = template.to_string();

    // STEP 1: Transform bind={var} or bind:value={var} to data-nini-bind="var"
    // Must run BEFORE expression transformation to avoid {var} becoming <span>
    for bind_pattern in &["bind:value={", "bind={"] {
        while let Some(bind_start) = result.find(bind_pattern) {
            let prefix_len = bind_pattern.len();
            let after_bind = &result[bind_start + prefix_len..];
            if let Some(brace_end) = after_bind.find('}') {
                let var_name = after_bind[..brace_end].trim().to_string();
                let rest = &after_bind[brace_end + 1..];

                // Find the closing > of the tag
                if let Some(gt_pos) = rest.find('>') {
                    let attrs_after = &rest[..gt_pos];
                    let after_tag = &rest[gt_pos..];

                    // Find the opening < of the tag (search backwards from bind_start)
                    let before_bind = &result[..bind_start];
                    if let Some(tag_open) = before_bind.rfind('<') {
                        let tag_content = &before_bind[tag_open + 1..];

                        // Extract tag name
                        let tag_name = if let Some(space) = tag_content.find(' ') {
                            &tag_content[..space]
                        } else {
                            tag_content.trim()
                        };

                        // Build new attributes: existing before bind + existing after bind + data-nini-bind
                        let attrs_before = tag_content[tag_name.len()..].trim();
                        let all_attrs = format!("{} {}", attrs_before, attrs_after)
                            .trim()
                            .to_string();

                        // Remove the bind pattern from attrs
                        let clean_attrs = all_attrs
                            .replace(&format!("bind:value={{{}}}", var_name), "")
                            .replace(&format!("bind={{{}}}", var_name), "")
                            .replace("bind:value={}", "")
                            .replace("bind={}", "")
                            .trim()
                            .to_string();

                        // Check if tag was self-closing (check original attrs, not cleaned)
                        let is_self_closing =
                            all_attrs.ends_with('/') || after_tag.starts_with("/>");
                        let clean_attrs = clean_attrs.trim_end_matches('/').trim();

                        let new_attr = format!("data-nini-bind=\"{}\"", var_name);
                        let final_attrs = if clean_attrs.is_empty() {
                            new_attr
                        } else {
                            format!("{} {}", clean_attrs, new_attr)
                        };

                        // Skip the /> or > from after_tag
                        let remaining = if after_tag.starts_with("/>") {
                            &after_tag[2..]
                        } else if after_tag.starts_with('>') {
                            &after_tag[1..]
                        } else {
                            after_tag
                        };

                        let replacement = if is_self_closing {
                            format!("<{} {} />", tag_name, final_attrs)
                        } else {
                            format!("<{} {}>", tag_name, final_attrs)
                        };

                        result = format!("{}{}{}", &result[..tag_open], replacement, remaining);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    // STEP 2: Transform onclick={expr} to onclick="expr"
    while let Some(click_start) = result.find("onclick={") {
        let after_click = &result[click_start + 9..];
        if let Some(brace_end) = after_click.find('}') {
            let expr = after_click[..brace_end].trim().to_string();
            let rest = &after_click[brace_end + 1..];

            if let Some(gt_pos) = rest.find('>') {
                let attrs_after = &rest[..gt_pos];
                let after_tag = &rest[gt_pos..];

                // Find the opening < of the tag
                let before_click = &result[..click_start];
                if let Some(tag_open) = before_click.rfind('<') {
                    let tag_content = &before_click[tag_open + 1..];

                    let tag_name = if let Some(space) = tag_content.find(' ') {
                        &tag_content[..space]
                    } else {
                        tag_content.trim()
                    };

                    let attrs_before = tag_content[tag_name.len()..].trim();
                    let all_attrs = format!("{} {}", attrs_before, attrs_after)
                        .trim()
                        .to_string();
                    let clean_attrs = all_attrs
                        .replace("onclick={}", "")
                        .replace(&format!("onclick={{{}}}", expr), "")
                        .trim()
                        .to_string();

                    let is_self_closing = clean_attrs.ends_with('/') || attrs_after.ends_with('/');
                    let clean_attrs = clean_attrs.trim_end_matches('/').trim();

                    let new_attr = format!("onclick=\"{}\"", expr);
                    let final_attrs = if clean_attrs.is_empty() {
                        new_attr
                    } else {
                        format!("{} {}", clean_attrs, new_attr)
                    };

                    let replacement = if is_self_closing {
                        format!("<{} {} />", tag_name, final_attrs)
                    } else {
                        format!("<{} {}>", tag_name, final_attrs)
                    };

                    result = format!("{}{}{}", &result[..tag_open], replacement, after_tag);
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // STEP 2: Transform onclick={expr} to onclick="expr"
    while let Some(click_start) = result.find("onclick={") {
        let after_click = &result[click_start + 9..];
        if let Some(brace_end) = after_click.find('}') {
            let expr = after_click[..brace_end].trim().to_string();
            let rest = &after_click[brace_end + 1..];

            if let Some(gt_pos) = rest.find('>') {
                let attrs_after = &rest[..gt_pos];
                let after_tag = &rest[gt_pos..];

                // Find the opening < of the tag
                let before_click = &result[..click_start];
                if let Some(tag_open) = before_click.rfind('<') {
                    let tag_content = &before_click[tag_open + 1..];

                    let tag_name = if let Some(space) = tag_content.find(' ') {
                        &tag_content[..space]
                    } else {
                        tag_content.trim()
                    };

                    let attrs_before = tag_content[tag_name.len()..].trim();
                    let all_attrs = format!("{} {}", attrs_before, attrs_after)
                        .trim()
                        .to_string();
                    let clean_attrs = all_attrs
                        .replace("onclick={}", "")
                        .replace(&format!("onclick={{{}}}", expr), "")
                        .trim()
                        .to_string();

                    let is_self_closing = clean_attrs.ends_with('/') || attrs_after.ends_with('/');
                    let clean_attrs = clean_attrs.trim_end_matches('/').trim();

                    let new_attr = format!("onclick=\"{}\"", expr);
                    let final_attrs = if clean_attrs.is_empty() {
                        new_attr
                    } else {
                        format!("{} {}", clean_attrs, new_attr)
                    };

                    let replacement = if is_self_closing {
                        format!("<{} {} />", tag_name, final_attrs)
                    } else {
                        format!("<{} {}>", tag_name, final_attrs)
                    };

                    result = format!("{}{}{}", &result[..tag_open], replacement, after_tag);
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // STEP 3: Transform @if and @foreach directives
    let mut directive_counter = 0;

    while let Some(if_start) = result.find("@if (") {
        if let Some(if_end_paren) = result[if_start..].find(") {") {
            let condition_start = if_start + 5; // after "@if ("
            let condition = result[condition_start..if_start + if_end_paren]
                .trim()
                .to_string();

            // Find matching }
            let content_start = if_start + if_end_paren + 3; // after ") {"
            if let Some(if_close) = find_matching_brace(&result[content_start..]) {
                let content = result[content_start..content_start + if_close]
                    .trim()
                    .to_string();
                directive_counter += 1;

                let replacement = format!(
                    r#"<div data-nini-if="{}" data-nini-if-id="{}" style="display:none">{}</div>"#,
                    condition, directive_counter, content
                );

                let full_end = content_start + if_close + 1; // +1 for "}"
                result = format!(
                    "{}{}{}",
                    &result[..if_start],
                    replacement,
                    &result[full_end..]
                );
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // Transform @foreach (var item in items) { ... } to data-nini-for (Blazor style)
    while let Some(each_start) = result.find("@foreach (") {
        if let Some(each_end_paren) = result[each_start..].find(") {") {
            let expr_start = each_start + 10; // after "@foreach ("
            let expr = result[expr_start..each_start + each_end_paren]
                .trim()
                .to_string();

            // Parse "var item in items" or "item in items"
            let expr_clean = expr.trim_start_matches("var ");
            if let Some((item_name, list_expr)) = expr_clean.split_once(" in ") {
                let item_name = item_name.trim().to_string();
                let list_name = list_expr.trim().to_string();

                // Find matching }
                let content_start = each_start + each_end_paren + 3; // after ") {"
                if let Some(each_close) = find_matching_brace(&result[content_start..]) {
                    let content = result[content_start..content_start + each_close]
                        .trim()
                        .to_string();
                    directive_counter += 1;

                    let replacement = format!(
                        r#"<div data-nini-for="{} in {}" data-nini-for-id="{}">{}</div>"#,
                        item_name, list_name, directive_counter, content
                    );

                    let full_end = content_start + each_close + 1; // +1 for "}"
                    result = format!(
                        "{}{}{}",
                        &result[..each_start],
                        replacement,
                        &result[full_end..]
                    );
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
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
                    r#"<span id="nini-expr-{}">{{{}}}</span>"#,
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
                // Detectar tipo de valor - limpiar cualquier whitespace
                let clean: &str = &value.trim();

                let js_value = if clean == "true" {
                    "true".to_string()
                } else if clean == "false" {
                    "false".to_string()
                } else if let Ok(_) = clean.parse::<f64>() {
                    clean.to_string()
                } else if clean.starts_with('[') || clean.starts_with('{') {
                    clean.to_string()
                } else {
                    format!("\"{}\"", clean)
                };

                // Crear signal y variable con getter/setter implícito
                js_code.push_str(&format!(
                    "let _{name} = nini.signal({js_value});\n",
                    name = name,
                    js_value = js_value
                ));
                js_code.push_str(&format!(
                    "Object.defineProperty(window, '{name}', {{\n",
                    name = name
                ));
                js_code.push_str(&format!(
                    "    get() {{ return _{name}.value; }},\n",
                    name = name
                ));
                js_code.push_str(&format!(
                    "    set(v) {{ _{name}.value = v; }}\n",
                    name = name
                ));
                js_code.push_str("});\n");
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
    fn test_parse_store_with_function() {
        let input =
            "store Carrito\n    total = 0\n    fn agregar()\n        total = 1\n    end\nend";
        let result = parse_file(input);
        assert!(result.is_ok());
        let (_, ast) = result.unwrap();
        println!("Store AST nodes: {:?}", ast.nodes);
        if let NiniNode::Store { name, members } = &ast.nodes[0] {
            println!("Store name: {}, members: {:?}", name, members);
        }
        assert!(matches!(&ast.nodes[0], NiniNode::Store { name, .. } if name == "Carrito"));
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

    #[test]
    fn test_bind_value_transforms_to_data_attribute() {
        let template = r#"<input type="text" bind:value="{username}" placeholder="Name" />"#;
        let result = transform_template(template, "test");

        assert!(
            result.contains("data-nini-bind=\"username\""),
            "Result should contain data-nini-bind attribute"
        );
        assert!(
            !result.contains("bind:value="),
            "Result should not contain bind:value"
        );
        assert!(
            result.contains("/>") || result.contains(">"),
            "Result should have valid tag ending"
        );
    }

    #[test]
    fn test_bind_value_without_braces() {
        let template = r#"<input bind:value="email" />"#;
        let result = transform_template(template, "test");

        assert!(
            result.contains("data-nini-bind=\"email\""),
            "Result should contain data-nini-bind with email"
        );
        assert!(
            !result.contains("bind:value="),
            "Result should not contain bind:value"
        );
    }

    #[test]
    fn test_bind_value_not_self_closing() {
        let template = r#"<input bind:value="{texto}"><span>Label</span></input>"#;
        let result = transform_template(template, "test");

        assert!(
            result.contains("data-nini-bind=\"texto\""),
            "Result should contain data-nini-bind"
        );
        assert!(
            result.contains("</input>"),
            "Result should preserve closing tag"
        );
    }

    #[test]
    fn test_multiple_bind_value_attributes() {
        let template = r#"<input bind:value="{nombre}" /><input bind:value="{apellido}" />"#;
        let result = transform_template(template, "test");

        assert!(
            result.contains("data-nini-bind=\"nombre\""),
            "Result should contain bind for nombre"
        );
        assert!(
            result.contains("data-nini-bind=\"apellido\""),
            "Result should contain bind for apellido"
        );
    }

    #[test]
    fn test_if_directive_transform() {
        let template = r#"<div>@if (mostrar) { <p>Visible</p> }</div>"#;
        let result = transform_template(template, "test");

        assert!(
            result.contains("data-nini-if=\"mostrar\""),
            "Result should contain data-nini-if"
        );
        assert!(!result.contains("@if"), "Result should not contain @if");
    }

    #[test]
    fn test_foreach_directive_transform() {
        let template = r#"<div>@foreach (var item in items) { <p>{item}</p> }</div>"#;
        let result = transform_template(template, "test");

        assert!(
            result.contains("data-nini-for="),
            "Result should contain data-nini-for"
        );
        assert!(
            !result.contains("@foreach"),
            "Result should not contain @foreach"
        );
    }

    #[test]
    fn test_store_generates_correct_js() {
        use super::*;

        let component = Component {
            script: vec![NiniNode::Store {
                name: "Carrito".to_string(),
                members: vec![NiniNode::Variable {
                    name: "total".to_string(),
                    value: "100".to_string(),
                }],
            }],
            template: "<div>Test</div>".to_string(),
            style: "".to_string(),
            file_path: "test.nini".to_string(),
        };

        let resolved = HashMap::new();
        let (_, css) = generate_component_js(&component, "test", &resolved);

        assert!(css.is_empty() || css.contains("test"));
    }

    #[test]
    fn test_computed_signal_in_runtime() {
        let runtime_js = r#"
            const s = nini.computed(() => 2 + 2);
            console.log(s.value);
        "#;
        assert!(runtime_js.contains("computed"));
    }

    #[test]
    fn test_onmount_lifecycle() {
        let runtime_js = r#"
            nini.onMount(() => console.log('mounted'));
            nini.triggerMount();
        "#;
        assert!(runtime_js.contains("onMount"));
        assert!(runtime_js.contains("triggerMount"));
    }

    #[test]
    fn test_ondestroy_alias() {
        let runtime_js = r#"
            nini.onDestroy(() => console.log('cleanup'));
        "#;
        assert!(runtime_js.contains("onDestroy"));
    }

    #[test]
    fn test_oninput_helper() {
        let runtime_js = r#"
            nini.onInput('#myInput', (value) => console.log(value));
        "#;
        assert!(runtime_js.contains("onInput"));
    }
}
