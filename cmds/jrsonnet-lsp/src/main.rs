use std::{fs::File, io::Write, path::PathBuf, str::FromStr};

use lsp_server::{Connection, ErrorCode, Message, Request, RequestId, Response};
use lsp_types::{
	notification::{DidChangeTextDocument, DidOpenTextDocument, Notification},
	request::{DocumentLinkRequest, HoverRequest},
	CompletionOptions, DidChangeTextDocumentParams, DidOpenTextDocumentParams, DocumentLink,
	DocumentLinkOptions, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
	TextDocumentSyncOptions, Url, WorkDoneProgressOptions,
};

fn main() {
	let mut log = File::create("test").unwrap();
	writeln!(log, "start").unwrap();
	let (connection, io_threads) = Connection::stdio();
	let capabilities = serde_json::to_value(&ServerCapabilities {
		completion_provider: Some(CompletionOptions::default()),
		definition_provider: Some(lsp_types::OneOf::Left(true)),
		document_link_provider: Some(DocumentLinkOptions {
			resolve_provider: Some(false),
			work_done_progress_options: WorkDoneProgressOptions::default(),
		}),
		hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
		text_document_sync: Some(TextDocumentSyncCapability::Options(
			TextDocumentSyncOptions {
				change: Some(TextDocumentSyncKind::FULL),
				open_close: Some(true),
				..TextDocumentSyncOptions::default()
			},
		)),
		..ServerCapabilities::default()
	})
	.expect("failed to convert capabilities to json");

	connection
		.initialize(capabilities)
		.expect("failed to initialize connection");

	writeln!(log, "initialized").unwrap();

	main_loop(&mut log, &connection).expect("main loop failed");

	io_threads.join().expect("failed to join io_threads");
}
fn main_loop(log: &mut File, connection: &Connection) -> anyhow::Result<()> {
	// let mut es = EvaluationState::default();
	// es.set_import_resolver(Box::new(FileImportResolver::default()));

	let reply = |response: Response| {
		connection
			.sender
			.send(Message::Response(response))
			.expect("failed to respond");
	};

	for msg in &connection.receiver {
		match msg {
			Message::Response(_) => (),
			Message::Request(req) => {
				if connection.handle_shutdown(&req)? {
					return Ok(());
				}
				if let Some((id, params)) = cast::<DocumentLinkRequest>(&req) {
					reply(Response::new_ok(id, <Vec<DocumentLink>>::new()));
				} else if let Some((id, params)) = cast::<HoverRequest>(&req) {
					let pos = params
						.text_document_position_params
						.text_document
						.uri
						.path();
					let buf = PathBuf::from_str(pos).unwrap();
				// let pos = es
				// 	.map_from_source_location(
				// 		&buf,
				// 		params.text_document_position_params.position.line as usize + 1,
				// 		params.text_document_position_params.position.character as usize + 1,
				// 	)
				// 	.unwrap();
				// let el = ExprLocation(buf.clone().into(), pos as usize, pos as usize);
				// let es2 = es.clone();
				// reply(Response::new_ok(
				// 	id,
				// 	Some(Hover {
				// 		range: None,
				// 		contents: HoverContents::Markup(MarkupContent {
				// 			kind: MarkupKind::Markdown,
				// 			value: es
				// 				.run_in_state_with_breakpoint(el, move || {
				// 					es2.reset_evaluation_state(&buf);
				// 					es2.import_file(&PathBuf::new(), &buf)?
				// 						.to_string()
				// 						.map(|_| ())
				// 				})
				// 				.unwrap()
				// 				.unwrap_or_else(|| Val::Null)
				// 				.value_type()
				// 				.to_string(),
				// 		}),
				// 	}),
				// ));
				} else {
					reply(Response::new_err(
						req.id,
						ErrorCode::MethodNotFound as i32,
						format!("unrecognized request {}", req.method),
					))
				}
				/*
				if let Some((id, params)) = cast::<DocumentLinkRequest>(&req) {
					 let links = handle_links(&files, params).unwrap_or_default();
					 reply(Response::new_ok(id, links));
				} else if let Some((id, params)) = cast::<GotoDefinition>(&req) {
					 if let Some(loc) = handle_goto(&files, params) {
						  reply(Response::new_ok(id, loc))
					 } else {
						  reply(Response::new_ok(id, ()))
					 }
				} else if let Some((id, params)) = cast::<HoverRequest>(&req) {
					 match handle_hover(&files, params) {
						  Some((range, markdown)) => {
								reply(Response::new_ok(
									 id,
									 Hover {
										  contents: HoverContents::Markup(MarkupContent {
												kind: MarkupKind::Markdown,
												value: markdown,
										  }),
										  range,
									 },
								));
						  }
						  None => {
								reply(Response::new_ok(id, ()));
						  }
					 }
				} else if let Some((id, params)) = cast::<Completion>(&req) {
					 let completions = handle_completion(&files, params.text_document_position)
						  .unwrap_or_default();
					 reply(Response::new_ok(id, completions));
				} else
				*/
			}
			Message::Notification(req) => {
				let mut handle = |text: String, uri: Url| {
					writeln!(log, "updated file: {:?}", uri).unwrap();
					let path = match PathBuf::from_str(uri.path()) {
						Ok(x) => x,
						Err(_) => return,
					};
					let (ast, errors) = jrsonnet_rowan_parser::parse(&text);
					// es.add_parsed_file(path.into(), text.into(), parsed)
					// 	.unwrap();
					writeln!(log, "parsed: {:?}", uri).unwrap();
				};

				match &*req.method {
					DidOpenTextDocument::METHOD => {
						let params: DidOpenTextDocumentParams =
							match serde_json::from_value(req.params) {
								Ok(x) => x,
								Err(_) => continue,
							};
						handle(params.text_document.text, params.text_document.uri);
					}
					DidChangeTextDocument::METHOD => {
						let params: DidChangeTextDocumentParams =
							match serde_json::from_value(req.params) {
								Ok(x) => x,
								Err(_) => continue,
							};
						for change in params.content_changes.into_iter() {
							handle(change.text, params.text_document.uri.clone());
						}
					}
					_ => continue,
				}
			}
		}
	}
	Ok(())
}
fn cast<R>(req: &Request) -> Option<(RequestId, R::Params)>
where
	R: lsp_types::request::Request,
	R::Params: serde::de::DeserializeOwned,
{
	req.clone().extract(R::METHOD).ok()
}
