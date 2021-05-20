use crate::steins::ai::cache::CACHE_ENG_STR;

use rust_bert::gpt_neo::{
    GptNeoConfigResources, GptNeoMergesResources, GptNeoModelResources, GptNeoVocabResources,
};
use rust_bert::{
  pipelines::common::ModelType,
  pipelines::text_generation::{TextGenerationConfig, TextGenerationModel},
  resources::{RemoteResource, Resource}
};
use tch::Device;

use once_cell::sync::Lazy;
use tokio::{ task, sync::Mutex };

use rand::seq::SliceRandom;

pub static NEOMODEL: Lazy<Mutex<TextGenerationModel>> =
  Lazy::new(||{
   let config_resource = Resource::Remote(RemoteResource::from_pretrained(
      GptNeoConfigResources::GPT_NEO_1_3B,
    ));
    let vocab_resource = Resource::Remote(RemoteResource::from_pretrained(
      GptNeoVocabResources::GPT_NEO_1_3B,
    ));
    let merges_resource = Resource::Remote(RemoteResource::from_pretrained(
      GptNeoMergesResources::GPT_NEO_1_3B,
    ));
    let model_resource = Resource::Remote(RemoteResource::from_pretrained(
      GptNeoModelResources::GPT_NEO_1_3B,
    ));
    let generate_config = TextGenerationConfig {
      model_type: ModelType::GPTNeo,
      model_resource,
      config_resource,
      vocab_resource,
      merges_resource,
      min_length: 10,
      max_length: 64,
      do_sample: false,
      early_stopping: true,
      num_beams: 4,
      num_return_sequences: 1,
      device: Device::Cpu,
      ..Default::default()
    };
    Mutex::new( TextGenerationModel::new(generate_config).unwrap() )
  });

pub async fn chat_neo(something: String) -> anyhow::Result<String> {
  info!("Generating GPT Neo response");
  let neo_model = NEOMODEL.lock().await;
  let cache_eng_vec = CACHE_ENG_STR.lock().await;
  let mut cache_slices = cache_eng_vec
                        .choose_multiple(&mut rand::thread_rng(), 64)
                        .map(AsRef::as_ref).collect::<Vec<&str>>();
  cache_slices.push(&something);
  task::spawn_blocking(move || {
    let output = neo_model.generate(&[something.as_str()], None);
    if output.is_empty() {
      error!("Failed to chat with Neo Model");
      // TODO: error should be here
      Ok(String::new())
    } else {
      // just get first maybe?
      let answer = &output[0];
      Ok(answer.clone())
    }
  }).await.unwrap()
}