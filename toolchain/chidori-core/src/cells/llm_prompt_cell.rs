use chidori_prompt_format::templating::templates::ChatModelRoles;
use std::collections::HashMap;
use std::env;
use tokio::runtime;
use crate::cells::{LLMPromptCell, SupportedModelProviders};
use crate::execution::primitives::operation::{InputItemConfiguation, InputSignature, InputType, OperationNode, OutputSignature};
use crate::execution::primitives::serialized_value::{RkyvSerializedValue as RKV, serialized_value_to_json_value};
use crate::library::std::ai::llm::{ChatCompletionReq, ChatCompletionRes, ChatModelBatch, MessageRole, TemplateMessage};


/// LLM Prompt Cells allow notebooks to invoke language models to generate text.
#[tracing::instrument]
pub fn llm_prompt_cell(cell: &LLMPromptCell) -> OperationNode {
    match cell {
        LLMPromptCell::Chat { provider, req, .. } => {
            let schema =
                chidori_prompt_format::templating::templates::analyze_referenced_partials(&req);
            let role_blocks =
                chidori_prompt_format::templating::templates::extract_roles_from_template(&req);

            let mut input_signature = InputSignature::new();
            for (key, value) in &schema.items {
                // input_signature.kwargs.insert(
                //     key.clone(),
                //     InputItemConfiguation {
                //         ty: Some(InputType::String),
                //         default: None,
                //     },
                // );
                input_signature.globals.insert(
                    key.clone(),
                    InputItemConfiguation {
                        ty: Some(InputType::String),
                        default: None,
                    },
                );
            }

            match provider {
                SupportedModelProviders::OpenAI => OperationNode::new(
                    input_signature,
                    OutputSignature::new(),
                    Box::new(move |x, _| {
                        let mut template_messages: Vec<TemplateMessage> = Vec::new();
                        let data = if let RKV::Object(m) = x {
                            serialized_value_to_json_value(
                                &m.get("globals").expect("should have globals key"),
                            )
                        } else {
                            serialized_value_to_json_value(&x)
                        };

                        for (a, b) in &role_blocks.clone() {
                            template_messages.push(TemplateMessage {
                                role: match a {
                                    ChatModelRoles::User => MessageRole::User,
                                    ChatModelRoles::System => MessageRole::System,
                                    ChatModelRoles::Assistant => MessageRole::Assistant,
                                },
                                content: chidori_prompt_format::templating::templates::render_template_prompt(&b.as_ref().unwrap().source, &data, &HashMap::new()).unwrap(),
                                name: None,
                                function_call: None,
                            });
                        }

                        // TODO: replace this to being fetched from configuration
                        let api_key = env::var("OPENAI_API_KEY").unwrap().to_string();
                        let c = crate::library::std::ai::llm::openai::OpenAIChatModel::new(api_key);
                        let result = {
                            // Create a new Tokio runtime or use an existing one
                            let rt = runtime::Runtime::new().unwrap();

                            // Use the runtime to block on the asynchronous operation
                            rt.block_on(async {
                                c.batch(ChatCompletionReq {
                                    model: "gpt-3.5-turbo".to_string(),
                                    template_messages,
                                    ..ChatCompletionReq::default()
                                })
                                .await
                            })
                        };
                        if let Ok(ChatCompletionRes { choices, .. }) = result {
                            RKV::String(choices[0].text.as_ref().unwrap().clone())
                        } else {
                            RKV::Null
                        }
                    }),
                ),
            }
        }
        LLMPromptCell::Completion { .. } => OperationNode::new(
            InputSignature::new(),
            OutputSignature::new(),
            Box::new(|x, _| x),
        ),
        LLMPromptCell::Embedding { .. } => OperationNode::new(
            InputSignature::new(),
            OutputSignature::new(),
            Box::new(|x, _| x),
        ),
    }
}