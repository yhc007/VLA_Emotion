use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlInputElement, File, FileList};
use crate::components::icons;

#[derive(Clone, Debug)]
pub struct UploadedDoc {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub size_bytes: u64,
    pub status: UploadStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UploadStatus {
    Pending,
    Uploading,
    Processing,
    Ready,
    Error(String),
}

#[component]
pub fn DocumentUpload(
    case_id: ReadSignal<Option<String>>,
    documents: ReadSignal<Vec<UploadedDoc>>,
    set_documents: WriteSignal<Vec<UploadedDoc>>,
) -> impl IntoView {
    let file_input_ref = NodeRef::<leptos::html::Input>::new();
    let (dragging, set_dragging) = signal(false);
    
    // 파일 선택 핸들러
    let on_file_select = move |_| {
        if let Some(input) = file_input_ref.get() {
            let input: HtmlInputElement = input.into();
            if let Some(files) = input.files() {
                handle_files(files, set_documents, case_id.get());
            }
        }
    };
    
    // 드래그 앤 드롭 핸들러
    let on_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        set_dragging.set(false);
        
        if let Some(dt) = ev.data_transfer() {
            if let Some(files) = dt.files() {
                handle_files(files, set_documents, case_id.get());
            }
        }
    };
    
    let on_drag_over = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        set_dragging.set(true);
    };
    
    let on_drag_leave = move |_: web_sys::DragEvent| {
        set_dragging.set(false);
    };
    
    // 파일 삭제
    let remove_doc = move |doc_id: String| {
        set_documents.update(|docs| {
            docs.retain(|d| d.id != doc_id);
        });
    };
    
    view! {
        <div class="grok-card rounded-md p-4">
            <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2 mb-3">
                {icons::layers()}
                "Documents"
            </h2>
            
            // 드래그 앤 드롭 영역
            <div
                class=move || {
                    let base = "border-2 border-dashed rounded-md p-4 text-center transition-colors cursor-pointer";
                    if dragging.get() {
                        format!("{} border-grok-blue bg-grok-blue/10", base)
                    } else {
                        format!("{} border-grok-border hover:border-grok-muted", base)
                    }
                }
                on:drop=on_drop
                on:dragover=on_drag_over
                on:dragleave=on_drag_leave
                on:click=move |_| {
                    if let Some(input) = file_input_ref.get() {
                        let input: HtmlInputElement = input.into();
                        input.click();
                    }
                }
            >
                <input
                    type="file"
                    multiple=true
                    accept=".pdf,.doc,.docx,.xls,.xlsx,.txt,.md,.png,.jpg,.jpeg"
                    class="hidden"
                    node_ref=file_input_ref
                    on:change=on_file_select
                />
                
                <div class="text-grok-muted mb-2">
                    {icons::upload()}
                </div>
                <p class="text-sm text-grok-muted">
                    "Drop files here or "
                    <span class="text-grok-blue">"browse"</span>
                </p>
                <p class="text-xs text-grok-muted mt-1 font-mono">
                    "PDF, DOC, XLS, TXT, Images"
                </p>
            </div>
            
            // 업로드된 파일 목록
            <div class="mt-3 space-y-2">
                {move || {
                    let docs = documents.get();
                    if docs.is_empty() {
                        view! {
                            <p class="text-xs text-grok-muted text-center py-2">"No documents"</p>
                        }.into_any()
                    } else {
                        view! {
                            {docs.into_iter().map(|doc| {
                                let doc_id = doc.id.clone();
                                let doc_id_for_remove = doc.id.clone();
                                
                                view! {
                                    <div class="flex items-center justify-between p-2 bg-grok-gray/50 rounded border border-grok-border group">
                                        <div class="flex items-center gap-2 min-w-0">
                                            <span class="text-sm">{get_file_icon(&doc.file_type)}</span>
                                            <div class="min-w-0">
                                                <p class="text-sm text-white truncate" title=doc.filename.clone()>
                                                    {doc.filename.clone()}
                                                </p>
                                                <p class="text-xs text-grok-muted font-mono">
                                                    {format_size(doc.size_bytes)}
                                                </p>
                                            </div>
                                        </div>
                                        <div class="flex items-center gap-2">
                                            {match &doc.status {
                                                UploadStatus::Uploading => view! {
                                                    <div class="w-4 h-4 border-2 border-grok-blue border-t-transparent rounded-full animate-spin" />
                                                }.into_any(),
                                                UploadStatus::Processing => view! {
                                                    <span class="text-xs text-yellow-400 font-mono">"Processing"</span>
                                                }.into_any(),
                                                UploadStatus::Ready => view! {
                                                    <span class="text-xs text-green-400">"OK"</span>
                                                }.into_any(),
                                                UploadStatus::Error(e) => view! {
                                                    <span class="text-xs text-red-400" title=e.clone()>"Error"</span>
                                                }.into_any(),
                                                _ => view! { <span /> }.into_any(),
                                            }}
                                            <button
                                                class="p-1 text-grok-muted hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                                                on:click=move |_| remove_doc(doc_id_for_remove.clone())
                                            >
                                                {icons::x()}
                                            </button>
                                        </div>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

fn handle_files(files: FileList, set_documents: WriteSignal<Vec<UploadedDoc>>, _case_id: Option<String>) {
    for i in 0..files.length() {
        if let Some(file) = files.get(i) {
            let doc = UploadedDoc {
                id: format!("doc-{}", js_sys::Date::now() as u64 + i as u64),
                filename: file.name(),
                file_type: get_file_type(&file.name()),
                size_bytes: file.size() as u64,
                status: UploadStatus::Ready, // TODO: 실제 업로드 구현
            };
            
            set_documents.update(|docs| {
                docs.push(doc);
            });
            
            // TODO: 실제 백엔드 업로드 구현
            // wasm_bindgen_futures::spawn_local(async move {
            //     upload_file(file, case_id).await;
            // });
        }
    }
}

fn get_file_type(filename: &str) -> String {
    filename
        .rsplit('.')
        .next()
        .unwrap_or("unknown")
        .to_lowercase()
}

fn get_file_icon(file_type: &str) -> &'static str {
    match file_type {
        "pdf" => "📄",
        "jpg" | "jpeg" | "png" | "gif" | "webp" => "🖼️",
        "xlsx" | "xls" => "📊",
        "docx" | "doc" => "📝",
        "txt" | "md" => "📃",
        _ => "📁",
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
