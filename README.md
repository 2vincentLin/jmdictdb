# jmdictdb

A Rust crate for building and searching a fast, local SQLite dictionary from the [JMdict](https://www.edrdg.org/jmdict/j_jmdict.html) XML file.

## Warning

The JMdict data has its own license, see [ELECTRONIC DICTIONARY RESEARCH AND DEVELOPMENT GROUP](https://www.edrdg.org/edrdg/licence.html)
## Features

- Converts JMdict XML to a normalized, searchable SQLite database.
- Fast search by kanji (keb) or reading (reb).
- Returns all senses, glosses, and part-of-speech tags for each entry.
- Simple API for integration into other Rust projects.
- Async, efficient, and minimal dependencies.

## Usage

1. **Build the database:**
   First, you need to download the XML file from [The JMDict Project](https://www.edrdg.org/jmdict/j_jmdict.html), I am using `JMdict_e`, which only contains Japanese to English. Put the file into `data` folder, if you want to use other file or place in other folder, modify the `DICT_URL`in `build_db.rs`

```rust
/// The URL to the JMdict XML file.
pub const DICT_URL: &str = "data/JMdict_e";
```


2. Use the provided CLI tool to parse JMdict XML and populate the SQLite DB. This will build the SQLite database in default location: `data/jmdict_e.db`, modify `DB_URL`in `dict_db.rs`if you prefer different location

 ```sh
 cargo run --bin build_db
 ```

3. **Search in your Rust code:**
```rust
use jmdict::dict_db::{DictDb, DB_URL};
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
    Ok()
}
```

4. use `contains_kanji` to tell which function to call, it's not perfect, but you can modify it
```rust
use jmdict::contains_kanji;
let query = "食べる";
if contains_kanji(query) {
	// Use keb search
	println!("it has kanji");
} else {
	// Use reb search
	println!("it has no kanji");
}
```
## Project Structure

- `src/models.rs`: Data models for JMdict entries.
- `src/dict_db.rs`: Database logic and search API.
- `src/bin/build_db.rs`: CLI tool to build the database.
- `data/`: Place your JMdict XML file here.

## License

MIT

## Credits

- [JMdict Project](https://www.edrdg.org/jmdict/j_jmdictdoc.html)
- [sqlx](https://github.com/launchbadge/sqlx)

## Why create this project

Because I have another project [dioxus-jlpt-flashcard: A Japanese flashcard app use Dioxus](https://github.com/2vincentLin/dioxus-jlpt-flashcard/tree/main) that use local AI to create a short Japanese story based on user practiced/unfamiliar words, and I need to show the translation in the tooltip, but it seems there's no ready-to-use crate, so I build one for myself.