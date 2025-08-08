
// use jmdict::dict_db::{DictDb, DB_URL, contains_kanji};
use jmdictdb::{DictDb, DB_URL, contains_kanji};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let dictdb = DictDb::connect(DB_URL).await?;

    let reb = "する";
    let keb = "食べる";


    let a = dictdb.search_entries_with_senses_by_reading(reb).await?;
    // println!("{:?}", a);

    println!("len: {}", a.len());

    for entry in a {
        println!("ent_seq: {:?}", entry.ent_seq); // this is the entry sequence
        println!("rebs: {:?}", entry.rebs); // the Japanese readings
        println!("kebs: {:?}", entry.kebs); // the kanji
        for sense in entry.senses {
            println!("  pos: {:?}", sense.pos); // the part of speech
            println!("  gloss: {:?}", sense.gloss); // the gloss (meaning)
        }
    }

    let a = dictdb.search_entries_with_senses_by_kanji(keb).await?;

    println!("len: {}", a.len());

    for entry in a {
        println!("ent_seq: {:?}", entry.ent_seq); // this is the entry sequence
        println!("rebs: {:?}", entry.rebs); // the Japanese readings
        println!("kebs: {:?}", entry.kebs); // the kanji
        for sense in entry.senses {
            println!("  pos: {:?}", sense.pos); // the part of speech
            println!("  gloss: {:?}", sense.gloss); // the gloss (meaning)
        }
    }

    let query = "食べる";
    if contains_kanji(query) {
        // Use keb search
        println!("it has kanji");
    } else {
        // Use reb search
        println!("it has no kanji");
    }

    Ok(())
}