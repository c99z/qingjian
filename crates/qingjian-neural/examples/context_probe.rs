//! 知微语境信号探针：JSONL 输入逐条输出条件分、空前文分、增益与真实耗时。
//! 用法：context_probe <知微模型> <样例.jsonl>；第二参数 --inspect 仅检查模型元数据。

use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Instant;

use qingjian_neural::CharScorer;
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: context_probe MODEL INPUT.jsonl|--inspect".into());
    }
    let started = Instant::now();
    let scorer = CharScorer::load(Path::new(&args[0]))?;
    eprintln!(
        "{}",
        json!({"load_ms": started.elapsed().as_secs_f64()*1000.0,
            "config": format!("{:?}", scorer.model().config()),
            "p2c": scorer.vocab().sep().is_some()})
    );
    if args[1] == "--inspect" {
        return Ok(());
    }
    if scorer.vocab().sep().is_some() {
        return Err("context probe requires a character language model".into());
    }
    let input = std::io::BufReader::new(std::fs::File::open(&args[1])?);
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    for line in input.lines() {
        let mut row: Value = serde_json::from_str(&line?)?;
        let before = row["before"].as_str().ok_or("missing before")?;
        let texts: Vec<&str> = row["texts"]
            .as_array()
            .ok_or("missing texts")?
            .iter()
            .map(|s| s.as_str().ok_or("invalid text"))
            .collect::<Result<_, _>>()?;
        let started = Instant::now();
        let empty = scorer.score("", &texts)?;
        let conditional = scorer.score(before, &texts)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let gain: Vec<f64> = conditional.iter().zip(&empty).map(|(a, b)| a - b).collect();
        row["empty"] = json!(empty);
        row["conditional"] = json!(conditional);
        row["gain"] = json!(gain);
        row["ms"] = json!(elapsed);
        writeln!(output, "{row}")?;
    }
    Ok(())
}
