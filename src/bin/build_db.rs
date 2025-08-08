use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::time::Instant;
use jmdictdb::{DictDb, JMDict, DB_URL};

/// The URL to the JMdict XML file.
pub const DICT_URL: &str = "data/JMdict_e";



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let total_start_time = Instant::now();
    println!("--- JMDict Parser Started ---");

    // --- 1. Load XML file into memory ---
    println!("\n[1/4] Loading JMdict file into memory...");
    let load_start = Instant::now();
    let file = File::open(DICT_URL)?;
    let mut reader = BufReader::new(file);
    let mut xml_content = String::new();
    reader.read_to_string(&mut xml_content)?;
    println!("     File loaded successfully. (took: {:.2?})", load_start.elapsed());

    // --- 2. Pre-process XML to handle custom entities ---
    println!("\n[2/4] Replacing XML entities using regex...");
    let replace_start = Instant::now();

    // Regex to find all <!ENTITY name "value"> definitions in the DTD.
    // This is necessary because the quick-xml parser doesn't handle DTDs by default.
    // It captures the entity name (e.g., "n") and its value (e.g., "noun").
    let entity_re = Regex::new(r#"<!ENTITY\s+([^\s]+)\s+"([^"]+)">"#)?;
    
    // Create a map to store entities for fast lookup, e.g., {"&n;" -> "noun"}
    let mut entity_map = HashMap::new();
    for cap in entity_re.captures_iter(&xml_content) {
        let name = &cap[1];
        let value = &cap[2];
        // We format it into the &name; format for replacement.
        entity_map.insert(format!("&{};", name), value.to_string());
    }

    // Replace all found entities in the XML content.
    // We clone the content to perform the replacements, keeping the original intact.
    let mut xml = xml_content; // No need to clone, we can consume xml_content
    for (entity, value) in &entity_map {
        xml = xml.replace(entity, value);
    }
    println!("     Found and replaced {} entities. (took: {:.2?})", entity_map.len(), replace_start.elapsed());
    
    // --- 3. Deserialize the cleaned XML into Rust structs ---
    println!("\n[3/4] Parsing XML into structs...");
    let parse_start = Instant::now();
    let dict: JMDict = quick_xml::de::from_str(&xml)?;
    println!("     Parsing complete. (took: {:.2?})", parse_start.elapsed());

    // --- 4. Print summary ---
    println!("Successfully parsed {} entries.", dict.entry.len());

    if let Some(entry) = dict.entry.get(0) {
        println!("First entry example: {:?}", entry);
    }
    
    println!("\nTotal time taken: {:.2?}", total_start_time.elapsed());

    // --- 5. Insert entries into the database ---

    println!("\n[4/4] Inserting entries into database, reset db if needed...");
    // Reset the database if it exists
    DictDb::reset_database(DB_URL).await?;
    let db = DictDb::connect(DB_URL).await?;
    let insert_start = Instant::now();

    db.insert_entries(&dict.entry).await?;

    println!("     Inserted {} entries. (took: {:.2?})", dict.entry.len(), insert_start.elapsed());

    Ok(())
}