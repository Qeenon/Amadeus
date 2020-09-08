#[cfg(not(feature = "w3g_rs"))] use tokio::process::Command;
#[cfg(not(feature = "w3g_rs"))] use serde_json::Value;

/*
subheader.replay_length_ms
metadata.map
slot_records[x].race_flag .status .player_id
player_records .id .name
reforged_player_records .id .name .clan
*/

#[cfg(feature="w3g_rs")]
fn analyze_rs(path: &str) -> jane_eyre::Result<String> {
  let p = w3grs::parse(String::from(path))?;
  Ok( String::from( p.metadata.map ) )
}

#[cfg(not(feature = "w3g_rs"))]
async fn analyze_js(path: &str) -> jane_eyre::Result<String> {
  let node_out = Command::new("sh")
        .arg("-c")
      //.arg(&format!("node js/w3gjs_parse.js {}", path))
        .arg(&format!("ts-node js/w3g_parse.ts {}", path))
        .output()
        .await?;
  let npm_stdout = String::from_utf8(node_out.stdout)?;
  if npm_stdout.is_empty() {
    let npm_stderr = String::from_utf8(node_out.stderr)?;
    info!("npm error: {}", &npm_stderr);
  }
  Ok(npm_stdout)
}

#[cfg(not(feature = "w3g_rs"))]
#[allow(clippy::type_complexity)]
fn prettify_analyze_js(j: &str) -> jane_eyre::Result<(String, Vec<(String, Vec<String>, Vec<u64>)>)> {
  let json : Value = serde_json::from_str(&j)?;
  let mut out = String::new();
  let mut pls = vec![];
  if let Some(map) = json.pointer("/map") {
    if let Some(file) = map.pointer("/file") {
      out = format!("**map**: {}\n", file.as_str().unwrap());
    }
    if let Some(checksum) = map.pointer("/checksum") { 
      let winner = checksum.as_str().unwrap();
      if !winner.is_empty() {
        out = format!("{}**winner**: {}\n", out, winner);
      }
    }
  }
  if let Some(players) = json.pointer("/players") {
    for playa in players.as_array().unwrap().iter() {
      let mut p = String::new();
      let mut s = String::new();
      let mut su = String::new();
      let mut sapm = vec![];
      if let Some(name) = playa.pointer("/name") {
        p = name.as_str().unwrap().to_string();
      }
      if let Some(race) = playa.pointer("/race") {
        s = format!("**race**: {}\n", race.as_str().unwrap());
      }
      if let Some(apm) = playa.pointer("/apm") {
        s = format!("{}**apm**: {}", s, apm.as_u64().unwrap());
      }
      if let Some(actions) = playa.pointer("/actions") {
        if let Some(timed) = actions.pointer("/timed") {
          let timed = timed.as_array().unwrap();
          for tapm in timed.iter() {
            sapm.push( tapm.as_u64().unwrap() );
          }
        }
      }
      if let Some(heroes) = playa.pointer("/heroes") {
        let heroz = heroes.as_array().unwrap();
        if !heroz.is_empty() {
          s = format!("{}\n\n*heroes*", s);
          for hero in heroz.iter() {
            if let Some(id) = hero.pointer("/id") {
              s = format!("{}\n**{}**", s, id.as_str().unwrap());
            }
            if let Some(level) = hero.pointer("/level") {
              s = format!("{} level {}", s, level.as_u64().unwrap());
            }
          }
        }
      }
      if let Some(units) = playa.pointer("/units") {
        if let Some(summary) = units.pointer("/summary") {
          if let Some(sum) = summary.as_object() {
            su = format!("{}\n*units*", su);
            for (k, v) in sum {
              su = format!("{}\n**{}**: {}", su, k, v);
            }
          }
        }
      }
      pls.push((p, vec![s, su], sapm));
    }
  }
  if let Some(duration) = json.pointer("/duration") {
    let dhuman = duration.as_u64().unwrap()/60/1000;
    out = format!("{}**duration**: {}min", out, dhuman);
  }
  Ok((out, pls))
}

#[cfg(feature="w3g_rs")]
pub async fn analyze(path: &str)
    -> jane_eyre::Result<(String, Vec<(String, Vec<String>, Vec<u64>)>)> {
  let replay_data = analyze_rs(path)?;
  Ok((replay_data, vec![]))
}

#[cfg(not(feature = "w3g_rs"))]
pub async fn analyze(path: &str)
    -> jane_eyre::Result<(String, Vec<(String, Vec<String>, Vec<u64>)>)> {
  let replay_data = analyze_js(path).await?;
  let pretty_daya = prettify_analyze_js(&replay_data)?;
  Ok(pretty_daya)
}

#[cfg(test)]
mod cyber_w3g_tests {
  use super::*;
  #[ignore]
  #[test]
  #[cfg(feature="w3g_rs")]
  fn parse_replay_test() {
    assert!( analyze_rs("example.w3g").is_ok() );
  }
  #[ignore]
  #[tokio::test(basic_scheduler)]
  #[cfg(not(feature = "w3g_rs"))]
  async fn my_test() -> Result<(), String> {
    if let Ok(replay_data) = analyze_js("example.w3g").await {
      assert!(!replay_data.is_empty());
      match prettify_analyze_js(&replay_data) {
        Ok((_p, ps)) => {
          assert_eq!(2, ps.len());
          Ok(())
        }, Err(err) => {
          Err(format!("Error parsing {:?}", err))
        }
      }
    } else {
      Err(String::from("Failed to get node output"))
    }
  }
}