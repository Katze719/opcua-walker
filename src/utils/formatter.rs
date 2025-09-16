use colored::*;
use opcua::types::*;
use base64::prelude::*;

pub fn format_node_id(node_id: &NodeId) -> String {
    match &node_id.identifier {
        Identifier::Numeric(id) => format!("ns={};i={}", node_id.namespace, id),
        Identifier::String(id) => format!("ns={};s={}", node_id.namespace, id),
        Identifier::Guid(id) => format!("ns={};g={}", node_id.namespace, id),
        Identifier::ByteString(id) => format!("ns={};b={}", node_id.namespace, 
            base64::prelude::BASE64_STANDARD.encode(id)),
    }
}

pub fn format_variant(variant: &Variant) -> String {
    match variant {
        Variant::Empty => "Empty".dimmed().to_string(),
        Variant::Boolean(val) => format!("{}", val),
        Variant::SByte(val) => format!("{}", val),
        Variant::Byte(val) => format!("{}", val),
        Variant::Int16(val) => format!("{}", val),
        Variant::UInt16(val) => format!("{}", val),
        Variant::Int32(val) => format!("{}", val),
        Variant::UInt32(val) => format!("{}", val),
        Variant::Int64(val) => format!("{}", val),
        Variant::UInt64(val) => format!("{}", val),
        Variant::Float(val) => format!("{}", val),
        Variant::Double(val) => format!("{}", val),
        Variant::String(val) => format!("\"{}\"", val.as_ref()),
        Variant::DateTime(val) => format!("{}", val.as_chrono().format("%Y-%m-%d %H:%M:%S")),
        Variant::Guid(val) => format!("{}", val),
        Variant::ByteString(val) => format!("ByteString({} bytes)", val.as_ref().len()),
        Variant::XmlElement(val) => format!("XmlElement({})", val),
        Variant::NodeId(val) => format_node_id(val),
        Variant::ExpandedNodeId(val) => format!("{}", val),
        Variant::StatusCode(val) => format!("StatusCode({})", val),
        Variant::QualifiedName(val) => format!("{}:{}", val.namespace_index, val.name.as_ref()),
        Variant::LocalizedText(val) => format!("\"{}\"", val.text.as_ref()),
        Variant::Array(array) => {
            if array.values.len() <= 3 {
                let items: Vec<String> = array.values.iter()
                    .map(|v| format_variant(v))
                    .collect();
                format!("[{}]", items.join(", "))
            } else {
                format!("[{} items]", array.values.len())
            }
        }
        _ => format!("{:?}", variant),
    }
}

pub fn format_status_code(status: &StatusCode) -> String {
    if status.is_good() {
        "✅ Good".green().to_string()
    } else if status.is_uncertain() {
        format!("⚠️  Uncertain ({})", status).yellow().to_string()
    } else {
        format!("❌ Bad ({})", status).red().to_string()
    }
}

pub fn format_node_class(node_class: NodeClass) -> String {
    let (icon, name, color) = match node_class {
        NodeClass::Object => ("📁", "Object", "blue"),
        NodeClass::Variable => ("📊", "Variable", "green"),
        NodeClass::Method => ("⚙️", "Method", "yellow"),
        NodeClass::ObjectType => ("📂", "ObjectType", "cyan"),
        NodeClass::VariableType => ("📈", "VariableType", "magenta"),
        NodeClass::ReferenceType => ("🔗", "ReferenceType", "white"),
        NodeClass::DataType => ("🏷️", "DataType", "bright_blue"),
        NodeClass::View => ("👁️", "View", "bright_green"),
        _ => ("❓", "Unknown", "dimmed"),
    };
    
    match color {
        "blue" => format!("{} {}", icon, name).blue(),
        "green" => format!("{} {}", icon, name).green(),
        "yellow" => format!("{} {}", icon, name).yellow(),
        "cyan" => format!("{} {}", icon, name).cyan(),
        "magenta" => format!("{} {}", icon, name).magenta(),
        "white" => format!("{} {}", icon, name).white(),
        "bright_blue" => format!("{} {}", icon, name).bright_blue(),
        "bright_green" => format!("{} {}", icon, name).bright_green(),
        _ => format!("{} {}", icon, name).dimmed(),
    }.to_string()
}

pub fn format_access_level(access_level: u8) -> String {
    let mut parts = Vec::new();
    
    if access_level & 0x01 != 0 {
        parts.push("Read".green().to_string());
    }
    if access_level & 0x02 != 0 {
        parts.push("Write".blue().to_string());
    }
    if access_level & 0x04 != 0 {
        parts.push("HistoryRead".yellow().to_string());
    }
    if access_level & 0x08 != 0 {
        parts.push("HistoryWrite".cyan().to_string());
    }
    
    if parts.is_empty() {
        "None".dimmed().to_string()
    } else {
        parts.join(" | ")
    }
}

pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}