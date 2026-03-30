// packages/nini-compiler/src/lib.rs

#[derive(Debug, PartialEq)]
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

// Parser para un constructo (variable o clase)
fn parse_construct(input: &str) -> IResult<&str, NiniNode> {
    let (input, _) = multispace0(input)?;
    alt((parse_variable, parse_class))(input)
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
#[derive(Debug)]
pub struct Component {
    pub script: Vec<NiniNode>, // AST del bloque <script>
    pub template: String,      // Contenido HTML del template (crudo)
    pub style: String,         // Contenido CSS del bloque <style> (ignorado por ahora)
}

#[derive(Debug)]
struct StyleRule {
    selector: String,
    properties: Vec<(String, String)>,
}

// Parser que separa el archivo en script, template y style
pub fn parse_component(input: &str) -> IResult<&str, Component> {
    // Buscar el bloque <script>...</script>
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("<script>")(input)?;
    let (input, script_content) = take_while(|c| c != '<')(input)?; // Tomar hasta que encuentre otro '<'
    let (input, _) = tag("</script>")(input)?;

    // Parsear el script content
    let (_, script_ast) = parse_file(script_content)
        .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))?;

    // El resto es template (y posiblemente <style>)
    let (input, template) = take_while(|_| true)(input)?; // Por ahora, todo el resto

    // Separar template y style (simple: buscar <style>)
    let (template, style) = if let Some(style_start) = template.find("<style>") {
        let (before, after) = template.split_at(style_start);
        let (style_content, _) = after.split_at(after.find("</style>").unwrap_or(after.len()));
        let style_content = &style_content[7..]; // remover "<style>"
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
        },
    ))
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
    let mut css = String::new();
    for rule in rules {
        // Prefijar el selector con la clase de scope
        let scoped_selector = format!(".{} {}", scope_class, rule.selector);
        css.push_str(&format!("{} {{\n", scoped_selector));
        for (prop, value) in &rule.properties {
            css.push_str(&format!("  {}: {};\n", prop, value));
        }
        css.push_str("}\n");
    }
    css
}

// Generador de código JavaScript para componentes
pub fn generate_component_js(component: &Component, scope_class: &str) -> (String, String) {
    let mut js_code = String::new();

    // Importar el runtime de Nini
    js_code.push_str("import { nini } from './nini-runtime-web/core.js';\n\n");

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
                        _ => {}
                    }
                }
                js_code.push_str("}\n");
            }
            _ => {}
        }
    }

    // Generar el template HTML
    // Por ahora, simplemente insertar el template crudo en #nini-app
    // y luego crear efectos para las expresiones.
    // Para simplificar, transformaremos el template reemplazando {expr} con spans con IDs.
    let template_html = transform_template(&component.template, scope_class);
    js_code.push_str(&format!(
        "document.getElementById('nini-app').innerHTML = `{}`;\n\n",
        template_html
    ));

    // Crear efectos para las expresiones
    // Necesitamos extraer las expresiones del template original.
    let parts = parse_template(&component.template);
    let mut effect_counter = 0;
    for part in parts {
        if let TemplatePart::Expression(expr) = part {
            effect_counter += 1;
            let span_id = format!("nini-expr-{}", effect_counter);
            // Reemplazar el span con ID correspondiente
            // (esto se hizo en transform_template, pero necesitamos suscribirnos)
            js_code.push_str(&format!(
                "nini.effect(() => {{\n    const el = document.getElementById('{}');\n    if (el) el.textContent = {}.value;\n}});\n",
                span_id, expr
            ));
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

    // Generar CSS scoped
    let rules = parse_style(&component.style);
    let css = generate_scoped_css(&rules, scope_class);

    (js_code, css)
}

// Transforma el template HTML reemplazando {expr} con spans con IDs únicas.
fn transform_template(template: &str, scope_class: &str) -> String {
    let mut result = String::new();
    let mut current = template;
    let mut expr_counter = 0;

    while !current.is_empty() {
        if let Some(start) = current.find('{') {
            // Texto antes de la llave
            if start > 0 {
                result.push_str(&current[..start]);
            }
            // Encontrar la llave de cierre
            if let Some(end) = current[start..].find('}') {
                expr_counter += 1;
                let expr = &current[start + 1..start + end];
                let span = format!(
                    r#"<span id="nini-expr-{}">{{{}.value}}</span>"#,
                    expr_counter,
                    expr.trim()
                );
                result.push_str(&span);
                current = &current[start + end + 1..];
            } else {
                // No hay cierre, tratar el resto como texto
                result.push_str(&current[start..]);
                break;
            }
        } else {
            // No más llaves
            result.push_str(current);
            break;
        }
    }
    // Envolver el resultado en un div con la clase de scope
    format!(r#"<div class="{}">{}</div>"#, scope_class, result)
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
}
