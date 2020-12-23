use crate::{
  types::{ options::IOptions
         , tracking::TrackingGame
         , twitch::Twitch
         , goodgame::GoodGameData },
  collections::team::players,
  common::{
    help::channel::channel_by_name,
    constants::{
      STREAMS_CHANNEL,
      LILYAL_CHANNEL,
      LILYAL
    }
  }
};

use serenity::{
  prelude::*,
  http::AttachmentType,
  model::{ id::ChannelId
         , id::GuildId }
};

use std::{
  borrow::Cow,
  collections::HashMap,
  time,
  sync::Arc
};

use chrono::DateTime;
use once_cell::sync::Lazy;
use rand::Rng;

pub static STREAMS: Lazy<Mutex<HashMap<u64, TrackingGame>>>
  = Lazy::new(|| Mutex::new(HashMap::new()));

async fn clear_channel(channel: ChannelId, ctx: &Context) {
  if let Ok(vec_msg) = channel.messages(&ctx, |g| g.limit(25)).await {
    let mut vec_id = Vec::new();
    for message in vec_msg {
      for embed in message.embeds {
        if let Some(title) = embed.title {
          if title != "FINISHED" {
            vec_id.push(message.id);
            break;
          }
        }
      }
    }
    if !vec_id.is_empty() {
      match channel.delete_messages(&ctx, vec_id.as_slice()).await {
        Ok(nothing)  => nothing,
        Err(err) => warn!("Failed to clean live messages {}", err),
      };
    }
  }
}

pub async fn activate_streamers_tracking(
                     ctx:       &Arc<Context>
                   , options:   &IOptions
                   , token:     String ) {

  let amadeus_guild_id = GuildId( options.amadeus_guild );

  let amadeus_storage =
    if let Ok(amadeus_channels) = amadeus_guild_id.channels(ctx).await {
      if let Some((ch, _)) = channel_by_name(&ctx, &amadeus_channels, "stream_pics").await {
        Some(*ch)
      } else { None }
    } else { None };

  set!{ ctx_clone     = Arc::clone(&ctx)
      , options_clone = options.clone() };

  clear_channel(STREAMS_CHANNEL, &ctx).await;
  clear_channel(LILYAL_CHANNEL, &ctx).await;

  tokio::spawn(async move {
    let mut streams_lock = STREAMS.lock().await;
    loop {
      let mut k_to_del : Vec<u64> = Vec::new();
      for (k, track) in streams_lock.iter_mut() {
        if track.passed_time < (60 * 24) {
          // for real it's 60 + some time for processing
          track.passed_time += 1;
        } else {
          k_to_del.push(*k);
        }
      }
      for ktd in k_to_del {
        warn!("stream {} out with timeout", ktd);
        streams_lock.remove(&ktd);
      }
      info!("streams check");
      for playa in players() {
        if let Ok(user) = ctx_clone.http.get_user(playa.discord).await {
          setm!{ twitch_live        = false
               , additional_fields  = Vec::new()
               , title              = String::new()
               , image              = None
               , em_url             = None };
          if playa.streams.is_some() {
            let streams = playa.streams.clone().unwrap();
            if streams.twitch.is_some() {
              let client = reqwest::Client::new();
              let getq = format!("https://api.twitch.tv/helix/streams?user_login={}", &streams.twitch.unwrap());
              if let Ok(res) = client
                .get(&getq)
                .header("Authorization", token.clone())
                .header("Client-ID", options_clone.twitch_client_id.clone())
                .send().await {
                match res.json::<Twitch>().await {
                  Ok(t) => {
                    if !t.data.is_empty() {
                      let twd = &t.data[0];
                      let url = format!("https://www.twitch.tv/{}", twd.user_name);
                      let pic = twd.thumbnail_url.replace("{width}", "800")
                                                 .replace("{height}", "450");
                      if twd.type_string == "live" {
                        let start =
                          if let Ok(tws) = DateTime::parse_from_rfc3339(&twd.started_at) {
                            // maybe MSK time?
                            let cet_time = tws.with_timezone(&chrono_tz::CET).time();
                            let time_format = "%k:%M";
                            cet_time.format(time_format).to_string()
                          } else {
                            twd.started_at.clone()
                          };
                        let t_d = format!("{}\n{}\nviewers: {}\nstarted: {} CET",
                                    twd.title, url, twd.viewer_count, start);
                        additional_fields.push(("Live on twitch", t_d, true));
                        title       = twd.title.clone();
                        image       = Some(pic);
                        em_url      = Some(url);
                        twitch_live = true;
                      }
                    }
                  }, Err(why) => {
                    error!("Failed to parse twitch structs\nrequest: {}\nerror: {:?}", &getq, why);
                  }
                }
              }
            }
            if streams.ggru.is_some() {
              let ggru = streams.ggru.clone().unwrap();
              let ggru_link = format!("http://api2.goodgame.ru/v2/streams/{}", &ggru);
              if let Ok(gg) = reqwest::get(&ggru_link).await {
                match gg.json::<GoodGameData>().await {
                  Ok(ggdata) => {
                    if ggdata.status == "Live" {
                      let url = format!("https://goodgame.ru/channel/{}", &ggru);
                      if twitch_live {
                        let viewers = format!( "viewers: {}\nin chat: {}"
                                             , ggdata.viewers, ggdata.users_in_chat );
                        let titurl =
                          format!("{}\n{}\n{}", &ggdata.channel.title, url, viewers);
                        additional_fields.push(("Live on ggru", titurl, true));
                      } else {
                        let viewers = format!( "viewers: {}\nin chat: {}"
                                             , ggdata.viewers, ggdata.users_in_chat );
                        additional_fields.push(("Live on ggru", viewers, true));
                        title  = ggdata.channel.title.clone();
                        image  = Some(ggdata.channel.thumb.clone());
                        em_url = Some(url);
                      }
                    }
                  }, Err(why) => {
                    error!("Failed to parse good game structs {:?}", why);
                  }
                };
              }
            }
          }
          if !additional_fields.is_empty() {
            if let Some(track) = streams_lock.get(&playa.discord) {
              if let Ok(mut msg) = ctx_clone.http.get_message(STREAMS_CHANNEL.0, track.tracking_msg_id[0]).await {
                let footer = if track.passed_time > 60 {
                    let hours: u32 = track.passed_time / 60;
                    let minutes = track.passed_time % 60;
                    format!("Passed: {} hours {} min", hours, minutes)
                  } else {
                    format!("Passed: {} min", track.passed_time)
                  };
                let mut fields = Vec::new();
                let mut img = None;
                let mut url = None;
                let mut color = (32, 32, 32);
                if !msg.embeds.is_empty() && !msg.embeds[0].fields.is_empty() {
                  for f in msg.embeds[0].fields.clone() {
                    fields.push((f.name, f.value, f.inline));
                  }
                  img = msg.embeds[0].image.clone();
                  url = msg.embeds[0].url.clone();
                  color = msg.embeds[0].colour.tuple();
                };
                let is_now_live = format!("{} is now live!", &user.name);
                if let Err(why) = msg.edit(&ctx_clone.http, |m| m
                  .embed(|e|  {
                    let mut e = e
                      .title(&title)
                      .colour(color)
                      .author(|a| a.icon_url(&user.face()).name(&is_now_live))
                      .footer(|f| f.text(&footer));
                    if !fields.is_empty() {
                      e = e.fields(fields.clone());
                    }
                    if let Some(some_img) = img.clone() {
                      e = e.image(some_img.url);
                    }
                    if let Some(some_url) = url.clone() {
                      e = e.url(some_url);
                    }
                    e
                  }
                )).await {
                  error!("Failed to edit stream msg {:?}", why);
                }
                if user.id.0 == LILYAL && track.tracking_msg_id.len() > 1 {
                  if let Ok(mut msg_lil) =
                      ctx_clone.http.get_message(LILYAL_CHANNEL.0, track.tracking_msg_id[1]).await {
                    if let Err(why) = msg_lil.edit(&ctx_clone.http, |m| m
                      .embed(|e|  {
                        let mut e = e
                          .title(&title)
                          .colour(color)
                          .author(|a| a.icon_url(&user.face()).name(&is_now_live))
                          .footer(|f| f.text(&footer));
                        if !fields.is_empty() {
                          e = e.fields(fields);
                        }
                        if let Some(some_img) = img {
                          e = e.image(some_img.url);
                        }
                        if let Some(some_url) = url {
                          e = e.url(some_url);
                        }
                        e
                      }
                    )).await {
                      error!("Failed to edit stream msg on lilyal {:?}", why);
                    }
                  }
                }
              }
            } else {
              let is_now_live = format!("{} started stream!", &user.name);
              if let Some(storage) = &amadeus_storage {
                if let Some(some_image) = &image {
                  if let Ok(response) = reqwest::get(some_image.as_str()).await {
                    if let Ok(bytes) = response.bytes().await {
                      let cow = AttachmentType::Bytes {
                        data: Cow::from(bytes.as_ref()),
                        filename: "stream_img.jpg".to_string()
                      };
                      match storage.send_message(&ctx_clone, |m| m.add_file(cow)).await {
                        Ok(msg) => {
                          if !msg.attachments.is_empty() {
                            let img_attachment = &msg.attachments[0];
                            image = Some(img_attachment.url.clone());
                          }
                        },
                        Err(why) => {
                          error!("Failed to download and post stream img {:?}", why);
                        }
                      };
                    }
                  }
                }
              }
              set! { red   = rand::thread_rng().gen_range(0..255)
                   , green = rand::thread_rng().gen_range(0..255)
                   , blue  = rand::thread_rng().gen_range(0..255) };
              match STREAMS_CHANNEL.send_message(&ctx_clone, |m| m
                .embed(|e| {
                  let mut e = e
                    .title(&title)
                    .colour((red, green, blue))
                    .author(|a| a.icon_url(&user.face()).name(&is_now_live));
                  if !additional_fields.is_empty() {
                    e = e.fields(additional_fields.clone());
                  }
                  if let Some(some_image) = image.clone() {
                    e = e.image(some_image);
                  }
                  if let Some(some_url) = em_url.clone() {
                    e = e.url(some_url);
                  }
                  e
                }
              )).await {
                Ok(msg_id) => {
                  let playa_for_stream = playa.clone();
                  streams_lock.insert(playa_for_stream.discord, TrackingGame {
                    tracking_msg_id: vec![msg_id.id.0],
                    passed_time: 0,
                    still_live: true,
                    players: vec![playa_for_stream], bets: vec![], fails: 0 }
                  );
                  if user.id.0 == LILYAL {
                    match LILYAL_CHANNEL.send_message(&ctx_clone, |m| m
                      .embed(|e| {
                        let mut e = e
                          .title(&title)
                          .colour((red, green, blue))
                          .author(|a| a.icon_url(&user.face()).name(&is_now_live));
                        if !additional_fields.is_empty() {
                          e = e.fields(additional_fields);
                        }
                        if let Some(some_image) = image {
                          e = e.image(some_image);
                        }
                        if let Some(some_url) = em_url {
                          e = e.url(some_url);
                        }
                        e
                      }
                    )).await {
                      Ok(msg_id_l) => {
                        if let Some(track) = streams_lock.get_mut(&playa.discord) {
                          track.tracking_msg_id.push(msg_id_l.id.0);
                        }
                      },
                      Err(why) => {
                        error!("Failed to post live match to lilyal {:?}", why);
                      }
                    }
                  }
                },
                Err(why) => {
                  error!("Failed to post live match {:?}", why);
                }
              }
            }
          } else if let Some(track) = streams_lock.get(&playa.discord) {
            if let Ok(mut msg) = ctx_clone.http.get_message(STREAMS_CHANNEL.0, track.tracking_msg_id[0]).await {
              let footer = if track.passed_time > 60 {
                  let hours: u32 = track.passed_time / 60;
                  let minutes = track.passed_time % 60;
                  format!("Passed: {} hours {} min", hours, minutes)
                } else {
                  format!("Passed: {} min", track.passed_time)
                };
              let mut fields = Vec::new();
              let mut img = None;
              let mut url = None;
              let mut color = (32, 32, 32);
              if !msg.embeds.is_empty() && !msg.embeds[0].fields.is_empty() {
                for f in msg.embeds[0].fields.clone() {
                  fields.push((f.name, f.value, f.inline));
                }
                img = msg.embeds[0].image.clone();
                url = msg.embeds[0].url.clone();
                color = msg.embeds[0].colour.tuple();
              };
              if let Err(why) = msg.edit(&ctx_clone.http, |m| m
                .embed(|e|  {
                  let mut e = e
                    .title("FINISHED")
                    .colour(color)
                    .author(|a| a.icon_url(&user.face()).name(&user.name))
                    .footer(|f| f.text(&footer));
                  if !fields.is_empty() {
                    e = e.fields(fields.clone());
                  }
                  if let Some(some_img) = img.clone() {
                    e = e.image(some_img.url);
                  }
                  if let Some(some_url) = url.clone() {
                    e = e.url(some_url);
                  }
                  e
                }
              )).await {
                error!("Failed to edit stream msg {:?}", why);
              }
              if user.id.0 == LILYAL && track.tracking_msg_id.len() > 1 {
                if let Ok(mut msgl) =
                    ctx_clone.http.get_message(LILYAL_CHANNEL.0, track.tracking_msg_id[1]).await {
                  if let Err(why) = msgl.edit(&ctx_clone.http, |m| m
                    .embed(|e|  {
                      let mut e = e
                        .title("FINISHED")
                        .colour(color)
                        .author(|a| a.icon_url(&user.face()).name(&user.name))
                        .footer(|f| f.text(&footer));
                      if !fields.is_empty() {
                        e = e.fields(fields);
                      }
                      if let Some(some_img) = img {
                        e = e.image(some_img.url);
                      }
                      if let Some(some_url) = url {
                        e = e.url(some_url);
                      }
                      e
                    }
                  )).await {
                    error!("Failed to edit stream msg on lilyal {:?}", why);
                  }
                }
              }
            }
            streams_lock.remove(&playa.discord);
          }
        }
        tokio::time::delay_for(time::Duration::from_millis(100)).await;
      }
      /* every minute */
      tokio::time::delay_for(time::Duration::from_secs(60)).await;
    }
  });
}
