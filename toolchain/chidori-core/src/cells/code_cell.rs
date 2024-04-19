use futures_util::FutureExt;
use chidori_static_analysis::language::Report;
use crate::cells::{CodeCell, SupportedLanguage};
use crate::execution::execution::ExecutionState;
use crate::execution::primitives::operation::{InputItemConfiguration, InputSignature, InputType, OperationFnOutput, OperationNode, OutputItemConfiguration, OutputSignature};

/// Code cells allow notebooks to evaluate source code in a variety of languages.
#[tracing::instrument]
pub fn code_cell(cell: &CodeCell) -> anyhow::Result<OperationNode> {
    match cell.language {
        SupportedLanguage::PyO3 => {
            // TODO: all callable functions should be exposed as their own OperationNodes
            //       these can be depended on throughout the graph as their own cells.
            let paths =
                chidori_static_analysis::language::python::parse::extract_dependencies_python(
                    &cell.source_code,
                )?;
            let report = chidori_static_analysis::language::python::parse::build_report(&paths);

            let (input_signature, output_signature) = signatures_from_report(&report);

            let cell = cell.clone();
            Ok(OperationNode::new(
                cell.name.clone(),
                input_signature,
                output_signature,
                Box::new(move |s, x, _, _| {
                    let cell = cell.clone();
                    // TODO: this needs to handle stdout and errors
                    let s = s.clone();
                    async move {
                        let result = crate::library::std::code::runtime_pyo3::source_code_run_python(
                            &s,
                            &cell.source_code,
                            &x,
                            &cell.function_invocation,
                        ).await.unwrap();
                        Ok(OperationFnOutput {
                            execution_state: None,
                            output: result.0,
                            stdout: result.1,
                            stderr: result.2,
                        })
                    }.boxed()
                }),
            ))
        }
        SupportedLanguage::Starlark => Ok(OperationNode::new(
            cell.name.clone(),
            InputSignature::new(),
            OutputSignature::new(),
            Box::new(|_, x, _, _| async move { Ok(OperationFnOutput::with_value(x)) }.boxed()),
        )),
        SupportedLanguage::Deno => {
            let paths =
                chidori_static_analysis::language::javascript::parse::extract_dependencies_js(
                    &cell.source_code,
                );
            let report = chidori_static_analysis::language::javascript::parse::build_report(&paths);

            let (input_signature, output_signature) = signatures_from_report(&report);

            let cell = cell.clone();
            Ok(OperationNode::new(
                cell.name.clone(),
                input_signature,
                output_signature,
                Box::new(move |_, x, _, _| {
                    // TODO: this needs to handle stdout and errors
                    let cell = cell.clone();
                    async move {
                        let result = tokio::task::spawn_blocking(move || {
                            let runtime = tokio::runtime::Runtime::new().unwrap();
                            let result = runtime.block_on(crate::library::std::code::runtime_deno::source_code_run_deno(
                                &ExecutionState::new(),
                                &cell.source_code,
                                &x,
                                &cell.function_invocation,
                            ));
                            match result {
                                Ok(v) => v.0,
                                Err(e) => panic!("{:?}", e),
                            }
                        }).await.unwrap();
                        Ok(OperationFnOutput::with_value(result))
                    }.boxed()
                }),
            ))
        }
    }
}

fn signatures_from_report(report: &Report) -> (InputSignature, OutputSignature) {
    let mut input_signature = InputSignature::new();
    for (key, value) in &report.cell_depended_values {
        input_signature.globals.insert(
            key.clone(),
            InputItemConfiguration {
                ty: Some(InputType::String),
                default: None,
            },
        );
    }

    let mut output_signature = OutputSignature::new();
    for (key, value) in &report.cell_exposed_values {
        output_signature.globals.insert(
            key.clone(),
            OutputItemConfiguration::Value,
        );
    }

    for (key, value) in &report.triggerable_functions {
        let mut input_signature = InputSignature::new();
        for (i, arg) in value.arguments.iter().enumerate() {
            input_signature.args.insert(arg.clone(), InputItemConfiguration {
                ty: Some(InputType::String),
                default: None,
            });
        }

        output_signature.functions.insert(
            key.clone(),
            OutputItemConfiguration::Function {
                input_signature,
                emit_event: vec![],
                trigger_on: vec![],
            },
        );
    }
    (input_signature, output_signature)
}


#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_code_cell() {


    }
}