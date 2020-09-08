use crate::{
  common::{ help::channel::channel_by_name
          , msg::channel_message
          },
  steins::cyber::w3g::analyze
};

use serenity::{
  prelude::*,
  model::{ channel::Attachment
         , id::GuildId
         , channel::Message, channel::ReactionType
         },
  http::AttachmentType,
  builder::CreateEmbed
};

use std::time::Duration;

use rand::Rng;

use async_std::{ fs::File, fs
               , prelude::* };

use plotters::prelude::*;

pub async fn replay_embed( ctx: &Context
                         , msg: &Message
                         , file: &Attachment
                         , amadeus_guild_id: &GuildId ) {
  let fname_apm = format!("{}_apm.png", &file.filename);
  if let Ok(bytes) = file.download().await {
    let mut fw3g = match File::create(&file.filename).await {
      Ok(replay) => replay,
      Err(why) => {
        error!("Error creating file: {:?}", why);
        channel_message(ctx, msg, "Error getting replay").await;
        return;
      }
    };
    if let Err(why) = fw3g.write_all(&bytes).await {
      error!("Error writing to file: {:?}", why);
      if let Err(why2) = fs::remove_file(&file.filename).await {
        error!("Error removing file: {:?}", why2);
      }
      return;
    }
    let _ = fw3g.sync_data().await;
    let data_maybe = analyze(&file.filename).await;
    if let Err(why) = data_maybe {
      error!("Corrupted replay file? {:?}", why);
      if let Err(why2) = fs::remove_file(&file.filename).await {
        error!("Error removing file: {:?}", why2);
      }
      return;
    }
    let (d, flds) = data_maybe.unwrap();
    let mut eb1 = CreateEmbed::default();
    let mut eb2 = CreateEmbed::default();
    let mut eb3 = CreateEmbed::default();
    let footer = format!("Uploaded by {}", msg.author.name);
    eb1.color(0xe535cc); eb2.color(0xe535cc); eb3.color(0xe535cc);
    eb1.title(&file.filename); eb2.title(&file.filename); eb3.title(&file.filename);
    eb1.description(&d); eb2.description("units"); eb3.description("apm");
    eb1.thumbnail("https://vignette.wikia.nocookie.net/steins-gate/images/0/07/Amadeuslogo.png");
    eb2.thumbnail("https://vignette.wikia.nocookie.net/steins-gate/images/0/07/Amadeuslogo.png");
    eb3.thumbnail("https://vignette.wikia.nocookie.net/steins-gate/images/0/07/Amadeuslogo.png");
    eb1.footer(|f| f.text(&footer));
    eb2.footer(|f| f.text(&footer));
    eb3.footer(|f| f.text(&footer));
    let mut max_apm = 0;
    if !flds.is_empty() {
      let mut fields1 = vec![];
      let mut fields2 = vec![];
      let mut fields3 = vec![];
      for (kk, vv, mut papm) in flds {
        if vv.len() > 1 {
          fields1.push((kk.clone(), vv[0].clone(), true));
          fields2.push((kk.clone(), vv[1].clone(), true));
        }
        if !papm.len() > 1 {
          // drop last value of apm, because it's "not full"
          papm.truncate(papm.len() - 1);
          let max = papm.iter().max().unwrap_or_else(|| &0);
          max_apm = std::cmp::max(max_apm, *max);
          fields3.push(
            ( kk.clone()
            , papm.into_iter().enumerate().map(|(i, x)| (i as f32, x as f64))
            )
          );
        }
      }
      let mut apm_image : Option<String> = None;
      if !fields3.is_empty() {
        let (_, first_amp_list) = &fields3[0];
        let len: f32 = first_amp_list.len() as f32 - 1_f32;
        //TODO: I need different text color here...
        { // because of Rc < > in BitMapBackend I need own scope here
          let root_area = BitMapBackend::new(&fname_apm, (1024, 768)).into_drawing_area();
          root_area.fill(&RGBColor(47, 49, 54)).unwrap(); //2f3136
          let mut cc = ChartBuilder::on(&root_area)
            .margin(5)
            .set_all_label_area_size(50)
            .build_cartesian_2d(0.0..len, 0.0..max_apm as f64)
            .unwrap();
          cc.configure_mesh()
            .label_style(("monospace", 14).into_font().color(&RGBColor(150, 150, 150)))
            .y_labels(10)
            .axis_style(&RGBColor(80, 80, 80))
            .draw().unwrap();
          for (k, plx) in fields3 {
            set! { red   = rand::thread_rng().gen_range(0, 255)
                  , green = rand::thread_rng().gen_range(0, 255)
                  , blue  = rand::thread_rng().gen_range(0, 255) };
            let mut color = RGBColor(red, green, blue);
            if red < 150 && green < 150 && blue < 150 {
              set! { red2   = rand::thread_rng().gen_range(100, 255)
                    , green2 = rand::thread_rng().gen_range(100, 255)
                    , blue2  = rand::thread_rng().gen_range(100, 255) };
              color = RGBColor(red2, green2, blue2);
            }
            cc.draw_series(LineSeries::new(plx, &color)).unwrap()
              .label(&k)
              .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &color));
          }
          cc.configure_series_labels()
            .border_style(&BLACK)
            .label_font(("monospace", 17).into_font().color(&RGBColor(200, 200, 200)))
            .draw().unwrap();
        }
        let amadeus_storage =
          if let Ok(amadeus_channels) = amadeus_guild_id.channels(&ctx).await {
            if let Some((ch, _)) = channel_by_name(&ctx, &amadeus_channels, "apm_pics").await {
              Some(*ch)
            } else { None }
          } else { None };
        if let Some(storage) = &amadeus_storage {
          match storage.send_message(&ctx, |m|
            m.add_file(AttachmentType::Path(std::path::Path::new(&fname_apm)))).await {
            Ok(msg) => {
              if !msg.attachments.is_empty() {
                let img_attachment = &msg.attachments[0];
                apm_image = Some(img_attachment.url.clone());
              }
            },
            Err(why) => {
              error!("Failed to download and post stream img {:?}", why);
            }
          };
        }
      }
      eb1.fields(fields1);
      eb2.fields(fields2);
      if let Some(apm) = apm_image {
        eb3.image(apm);
      }
    }
    let embeds = vec![ eb1, eb3, eb2 ];
    if let Ok(mut bot_msg) = msg.channel_id.send_message(&ctx, |m| {
                                m.embed(|e| { e.0 = embeds[0].0.clone(); e })
                              }).await {
      let mut page : usize = 0;
      let left = ReactionType::Unicode(String::from("⬅️"));
      let right = ReactionType::Unicode(String::from("➡️"));
      let _ = bot_msg.react(&ctx, left).await;
      let _ = bot_msg.react(&ctx, right).await;
      loop {
        if let Some(reaction) =
          &bot_msg.await_reaction(&ctx)
                  .timeout(Duration::from_secs(360)).await {
          let emoji = &reaction.as_inner_ref().emoji;
          match emoji.as_data().as_str() {
            "⬅️" => { 
              if page != 0 {
                page -= 1;
              }
            },
            "➡️" => { 
              if page != 2 {
                page += 1;
              }
            },
            _ => (),
          }
          if let Err(err) = bot_msg.edit(&ctx, |m|
            m.embed(|mut e| {
              e.0 = embeds[page].0.clone(); e
            })
          ).await {
            error!("Shit happens {:?}", err);
          }
          let _ = reaction.as_inner_ref().delete(&ctx).await;
        } else {
          let _ = bot_msg.delete_reactions(&ctx).await;
          break;
        };
      }
    } else {
      error!("Failed to post replay analyze data");
    }
    if let Err(why1) = fs::remove_file(&fname_apm).await {
      error!("Error removing apm png {:?}", why1);
    }
    if let Err(why2) = fs::remove_file(&file.filename).await {
      error!("Error removing file: {:?}", why2);
    }
  }
}