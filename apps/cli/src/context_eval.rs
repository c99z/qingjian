//! 固定前文的候选评测：首帧、稳定帧和等待时间分开记录，不提交或学习。

use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Instant;

use qingjian_core::Engine;
use serde_json::{Value, json};

pub fn run(engine: &mut Engine, input: &Path, output: &Path) -> std::io::Result<()> {
    let reader = std::io::BufReader::new(std::fs::File::open(input)?);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut writer = std::io::BufWriter::new(file);
    for line in reader.lines() {
        let mut row: Value = serde_json::from_str(&line?)?;
        let keys = row["keys"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("missing keys"))?;
        let before = row["before"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("missing before"))?;
        engine.clear();
        engine.invalidate_context();
        engine.set_rescoring_context(Some(before.to_owned()));
        for ch in keys.chars() {
            engine.push(ch);
        }
        let start = Instant::now();
        let first = engine.query().map_err(std::io::Error::other)?;
        let first_ms = start.elapsed().as_secs_f64() * 1000.0;
        let settled = crate::rescoring::settle(engine);
        let final_query = if settled {
            engine.query().map_err(std::io::Error::other)?
        } else {
            first.clone()
        };
        let stable_ms = start.elapsed().as_secs_f64() * 1000.0;
        row["first"] = json!(
            first
                .candidates
                .items
                .iter()
                .take(12)
                .map(|c| &c.text)
                .collect::<Vec<_>>()
        );
        row["final"] = json!(
            final_query
                .candidates
                .items
                .iter()
                .take(12)
                .map(|c| &c.text)
                .collect::<Vec<_>>()
        );
        row["first_ms"] = json!(first_ms);
        row["stable_ms"] = json!(stable_ms);
        row["settled"] = json!(settled);
        writeln!(writer, "{row}")?;
    }
    writer.flush()
}
