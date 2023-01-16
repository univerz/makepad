
use {
    std::rc::Rc,
    std::cell::RefCell,
    crate::{
        makepad_live_id::*,
        makepad_wasm_bridge::{WasmDataU8, FromWasmMsg, ToWasmMsg, FromWasm, ToWasm},
        os::{
            web_midi::WebMidiAccess,
            web_browser::{
                from_wasm::*,
                to_wasm::*,
            },
        },
        thread::Signal,
        window::{
            CxWindowPool
        },
        event::{
            ToWasmMsgEvent,
            WebSocket,
            WebSocketErrorEvent,
            WebSocketMessageEvent,
            WebSocketAutoReconnect,
            Event,
            XRInput,
            TextCopyEvent,
            TimerEvent,
            MouseDownEvent,
            MouseMoveEvent,
            MouseUpEvent,
            TouchState,
            TouchUpdateEvent,
            ScrollEvent,
            WindowGeom,
            WindowGeomChangeEvent
        },
        cx_api::{CxOsApi, CxOsOp},
        cx::{Cx},
    }
};

impl Cx {
    
    // incoming to_wasm. There is absolutely no other entrypoint
    // to general rust codeflow than this function. Only the allocators and init
    pub fn process_to_wasm(&mut self, msg_ptr: u32) -> u32 {
        
        let mut to_wasm_msg = ToWasmMsg::take_ownership(msg_ptr);
        
        self.os.from_wasm = Some(FromWasmMsg::new());
        let mut to_wasm = to_wasm_msg.as_ref();
        let mut is_animation_frame = false;
        while !to_wasm.was_last_block() {
            let block_id = LiveId(to_wasm.read_u64());
            let skip = to_wasm.read_block_skip();
            match block_id {
                live_id!(ToWasmGetDeps) => { // fetch_deps
                    let tw = ToWasmGetDeps::read_to_wasm(&mut to_wasm);
                    self.cpu_cores = tw.cpu_cores as usize;
                    self.gpu_info.init_from_info(
                        tw.gpu_info.min_uniform_vectors,
                        tw.gpu_info.vendor,
                        tw.gpu_info.renderer
                    );
                    self.platform_type = tw.browser_info.into();
                    self.xr_capabilities = tw.xr_capabilities.into();
                    let mut deps = Vec::<String>::new();
                    for (path, _) in &self.dependencies {
                        deps.push(path.to_string());
                    }
                    
                    self.os.from_wasm(
                        FromWasmLoadDeps {deps}
                    );
                },
                
                live_id!(ToWasmInit) => {
                    let tw = ToWasmInit::read_to_wasm(&mut to_wasm);
                    
                    for dep_in in tw.deps {
                        if let Some(dep) = self.dependencies.get_mut(&dep_in.path) {
                            
                            dep.data = Some(Ok(dep_in.data.into_vec_u8()))
                        }
                    }
                    self.os.window_geom = tw.window_info.into();
                    // start the signal poll timer
                    self.os.from_wasm(FromWasmStartTimer{timer_id: 0.0, repeats: true, interval: 0.016});
                    //self.default_inner_window_size = self.os.window_geom.inner_size;
                    
                    self.call_event_handler(&Event::Construct);
                    //self.platform.from_wasm(FromWasmCreateThread{thread_id:1});
                },
                
                live_id!(ToWasmResizeWindow) => {
                    let tw = ToWasmResizeWindow::read_to_wasm(&mut to_wasm);
                    let old_geom = self.os.window_geom.clone();
                    let new_geom = tw.window_info.into();
                    if old_geom != new_geom {
                        self.os.window_geom = new_geom.clone();
                        let id_zero = CxWindowPool::id_zero();
                        self.windows[id_zero].window_geom = new_geom.clone();
                        self.call_event_handler(&Event::WindowGeomChange(WindowGeomChangeEvent {
                            window_id: id_zero,
                            old_geom: old_geom,
                            new_geom: new_geom
                        }));
                        self.redraw_all();
                    }
                }
                
                live_id!(ToWasmAnimationFrame) => {
                    let tw = ToWasmAnimationFrame::read_to_wasm(&mut to_wasm);
                    is_animation_frame = true;
                    if self.new_next_frames.len() != 0 {
                        self.call_next_frame_event(tw.time);
                    }
                }
                
                live_id!(ToWasmTouchUpdate) => {
                    let e: TouchUpdateEvent = ToWasmTouchUpdate::read_to_wasm(&mut to_wasm).into();
                    for touch in &e.touches{
                        if let TouchState::Start = touch.state{
                            self.fingers.process_tap_count(touch.abs, e.time);
                        }
                    }
                    let e = Event::TouchUpdateEvent(e);
                    self.call_event_handler(&e);
                    let e = if let Event::TouchUpdateEvent(e) = e{e}else{panic!()};
                    for touch in &e.touches{
                        let digit_id = live_id_num!(touch, touch.uid).into();
                        match touch.state{
                            TouchState::Stop=>{
                                self.fingers.release_digit(digit_id);
                                self.fingers.remove_hover(digit_id);
                            }
                            TouchState::Start | TouchState::Move | TouchState::Stable =>{
                                self.fingers.cycle_hover_area(digit_id);
                            }
                        }
                    }
                    self.fingers.move_captures();
                }
                
                live_id!(ToWasmMouseDown) => {
                    let e: MouseDownEvent = ToWasmMouseDown::read_to_wasm(&mut to_wasm).into();
                    self.fingers.process_tap_count(e.abs, e.time);
                    self.fingers.mouse_down(e.button);
                    self.call_event_handler(&Event::MouseDown(e))
                }
                
                live_id!(ToWasmMouseMove) => {
                    let e: MouseMoveEvent = ToWasmMouseMove::read_to_wasm(&mut to_wasm).into();
                    self.call_event_handler(&Event::MouseMove(e.into()));
                    self.fingers.cycle_hover_area(live_id!(mouse).into());
                    self.fingers.move_captures();
                }
                
                live_id!(ToWasmMouseUp) => {
                    let e: MouseUpEvent = ToWasmMouseUp::read_to_wasm(&mut to_wasm).into();
                    let button = e.button;
                    self.call_event_handler(&Event::MouseUp(e.into()));
                    self.fingers.mouse_up(button);
                }
                
                live_id!(ToWasmScroll) => {
                    let e: ScrollEvent = ToWasmScroll::read_to_wasm(&mut to_wasm).into();
                    self.call_event_handler(&Event::Scroll(e.into()));
                }
                
                live_id!(ToWasmKeyDown) => {
                    let tw = ToWasmKeyDown::read_to_wasm(&mut to_wasm);
                    self.keyboard.process_key_down(tw.key.clone().into());
                    self.call_event_handler(&Event::KeyDown(tw.key.into()));
                }
                
                live_id!(ToWasmKeyUp) => {
                    let tw = ToWasmKeyUp::read_to_wasm(&mut to_wasm);
                    self.keyboard.process_key_up(tw.key.clone().into());
                    self.call_event_handler(&Event::KeyUp(tw.key.into()));
                }
                
                live_id!(ToWasmTextInput) => {
                    let tw = ToWasmTextInput::read_to_wasm(&mut to_wasm);
                    self.call_event_handler(&Event::TextInput(tw.into()));
                }
                
                live_id!(ToWasmTextCopy) => {
                    let response = Rc::new(RefCell::new(None));
                    self.call_event_handler(&Event::TextCopy(TextCopyEvent {
                        response: response.clone()
                    }));
                    let response = response.borrow_mut().take();
                    if let Some(response) = response {
                        self.os.from_wasm(FromWasmTextCopyResponse {response});
                    }
                }
                
                live_id!(ToWasmTimerFired) => {
                    let tw = ToWasmTimerFired::read_to_wasm(&mut to_wasm);
                    if tw.timer_id == 0{ // signal poll timer
                        if Signal::check_and_clear_ui_signal(){
                            self.handle_media_signals();
                            self.call_event_handler(&Event::Signal);
                        }
                    }
                    else{
                        self.call_event_handler(&Event::Timer(TimerEvent {
                            timer_id: tw.timer_id as u64
                        }));
                    }
                }
                
                live_id!(ToWasmAppGotFocus) => {
                    self.call_event_handler(&Event::AppGotFocus);
                }
                
                live_id!(ToWasmAppLostFocus) => {
                    self.call_event_handler(&Event::AppLostFocus);
                }
                
                live_id!(ToWasmXRUpdate) => {
                    let tw = ToWasmXRUpdate::read_to_wasm(&mut to_wasm);
                    let event = Event::XRUpdate(
                        tw.into_xrupdate_event(self.os.xr_last_inputs.take())
                    );
                    self.call_event_handler(&event);
                    if let Event::XRUpdate(event) = event {
                        self.os.xr_last_inputs = Some(event.inputs);
                    }
                }
                
                live_id!(ToWasmRedrawAll) => {
                    self.redraw_all();
                }
                
                live_id!(ToWasmPaintDirty) => {
                    let main_pass_id = self.windows[CxWindowPool::id_zero()].main_pass_id.unwrap();
                    self.passes[main_pass_id].paint_dirty = true;
                }
                
                live_id!(ToWasmWebSocketClose) => {
                    let tw = ToWasmWebSocketClose::read_to_wasm(&mut to_wasm);
                    let web_socket = WebSocket(tw.web_socket_id as u64);
                    self.call_event_handler(&Event::WebSocketClose(web_socket));
                }
                
                live_id!(ToWasmWebSocketOpen) => {
                    let tw = ToWasmWebSocketOpen::read_to_wasm(&mut to_wasm);
                    let web_socket = WebSocket(tw.web_socket_id as u64);
                    self.call_event_handler(&Event::WebSocketOpen(web_socket));
                }
                
                live_id!(ToWasmWebSocketError) => {
                    let tw = ToWasmWebSocketError::read_to_wasm(&mut to_wasm);
                    let web_socket = WebSocket(tw.web_socket_id as u64);
                    self.call_event_handler(&Event::WebSocketError(WebSocketErrorEvent {
                        web_socket,
                        error: tw.error,
                    }));
                }
                
                live_id!(ToWasmWebSocketMessage) => {
                    let tw = ToWasmWebSocketMessage::read_to_wasm(&mut to_wasm);
                    let web_socket = WebSocket(tw.web_socket_id as u64);
                    self.call_event_handler(&Event::WebSocketMessage(WebSocketMessageEvent {
                        web_socket,
                        data: tw.data.into_vec_u8()
                    }));
                }
                
                msg_id => {
                    // swap the message into an event to avoid a copy
                    let offset = to_wasm.u32_offset;
                    drop(to_wasm);
                    let event = Event::ToWasmMsg(ToWasmMsgEvent {id: msg_id, msg: to_wasm_msg, offset});
                    self.call_event_handler(&event);
                    // and swap it back
                    if let Event::ToWasmMsg(ToWasmMsgEvent {msg, ..}) = event {to_wasm_msg = msg}else {panic!()};
                    to_wasm = to_wasm_msg.as_ref();
                }
            };
            to_wasm.block_skip(skip);
        };
        
        if is_animation_frame {
            if self.need_redrawing() {
                self.call_draw_event();
                self.webgl_compile_shaders();
            }
            self.handle_repaint();
        }
        
        self.handle_platform_ops();
        self.handle_media_signals();
        
        if self.any_passes_dirty() || self.need_redrawing() || self.new_next_frames.len() != 0 {
            self.os.from_wasm(FromWasmRequestAnimationFrame {});
        }
        
        //return wasm pointer to caller
        self.os.from_wasm.take().unwrap().release_ownership()
    }
    
    // empty stub
    pub fn event_loop<F>(&mut self, mut _event_handler: F)
    where F: FnMut(&mut Cx, Event) {
    }
    
    fn handle_platform_ops(&mut self) {
        while let Some(op) = self.platform_ops.pop() {
            match op {
                CxOsOp::CreateWindow(window_id) => {
                    let window = &mut self.windows[window_id];
                    self.os.from_wasm(FromWasmSetDocumentTitle {
                        title: window.create_title.clone()
                    });
                    window.window_geom = self.os.window_geom.clone();
                    window.is_created = true;
                },
                CxOsOp::CloseWindow(_window_id) => {
                },
                CxOsOp::MinimizeWindow(_window_id) => {
                },
                CxOsOp::MaximizeWindow(_window_id) => {
                },
                CxOsOp::RestoreWindow(_window_id) => {
                },
                CxOsOp::FullscreenWindow(_window_id) => {
                    self.os.from_wasm(FromWasmFullScreen {});
                },
                CxOsOp::NormalizeWindow(_window_id) => {
                    self.os.from_wasm(FromWasmNormalScreen {});
                }
                CxOsOp::SetTopmost(_window_id, _is_topmost) => {
                    todo!()
                }
                CxOsOp::XrStartPresenting => {
                    self.os.from_wasm(FromWasmXrStartPresenting {});
                },
                CxOsOp::XrStopPresenting => {
                    self.os.from_wasm(FromWasmXrStopPresenting {});
                },
                CxOsOp::ShowTextIME(area, pos) => {
                    let pos = area.get_clipped_rect(self).pos + pos;
                    self.os.from_wasm(FromWasmShowTextIME {x: pos.x, y: pos.y});
                },
                CxOsOp::HideTextIME => {
                    self.os.from_wasm(FromWasmHideTextIME {});
                },
                
                CxOsOp::SetCursor(cursor) => {
                    self.os.from_wasm(FromWasmSetMouseCursor::new(cursor));
                },
                CxOsOp::StartTimer {timer_id, interval, repeats} => {
                    self.os.from_wasm(FromWasmStartTimer {
                        repeats,
                        interval,
                        timer_id: timer_id as f64,
                    });
                },
                CxOsOp::StopTimer(timer_id) => {
                    self.os.from_wasm(FromWasmStopTimer {
                        id: timer_id as f64,
                    });
                },
                CxOsOp::StartDragging(_dragged_item) => {
                }
                CxOsOp::UpdateMenu(_menu) => {
                }
            }
        }
    }
    
}


impl CxOsApi for Cx {
    fn init(&mut self) {
        self.live_expand();
        self.live_scan_dependencies();
        
        self.os.append_to_wasm_js(&[
            ToWasmGetDeps::to_string(),
            ToWasmInit::to_string(),
            ToWasmResizeWindow::to_string(),
            ToWasmAnimationFrame::to_string(),
            
            ToWasmTouchUpdate::to_string(),
            ToWasmMouseDown::to_string(),
            ToWasmMouseMove::to_string(),
            ToWasmMouseUp::to_string(),
            ToWasmScroll::to_string(),
            
            ToWasmKeyDown::to_string(),
            ToWasmKeyUp::to_string(),
            ToWasmTextInput::to_string(),
            ToWasmTextCopy::to_string(),
            ToWasmTimerFired::to_string(),
            ToWasmPaintDirty::to_string(),
            ToWasmRedrawAll::to_string(),
            ToWasmXRUpdate::to_string(),
            ToWasmAppGotFocus::to_string(),
            ToWasmAppLostFocus::to_string(),
            ToWasmWebSocketOpen::to_string(),
            ToWasmWebSocketClose::to_string(),
            ToWasmWebSocketError::to_string(),
            ToWasmWebSocketMessage::to_string(),
        
            ToWasmMidiInputData::to_string(),
            ToWasmMidiInputList::to_string(),
        ]);
        
        self.os.append_from_wasm_js(&[
            FromWasmLoadDeps::to_string(),
            FromWasmStartTimer::to_string(),
            FromWasmStopTimer::to_string(),
            FromWasmFullScreen::to_string(),
            FromWasmNormalScreen::to_string(),
            FromWasmRequestAnimationFrame::to_string(),
            FromWasmSetDocumentTitle::to_string(),
            FromWasmSetMouseCursor::to_string(),
            FromWasmTextCopyResponse::to_string(),
            FromWasmShowTextIME::to_string(),
            FromWasmHideTextIME::to_string(),
            FromWasmCreateThread::to_string(),
            FromWasmWebSocketOpen::to_string(),
            FromWasmWebSocketSend::to_string(),
            FromWasmXrStartPresenting::to_string(),
            FromWasmXrStopPresenting::to_string(),
            
            FromWasmCompileWebGLShader::to_string(),
            FromWasmAllocArrayBuffer::to_string(),
            FromWasmAllocIndexBuffer::to_string(),
            FromWasmAllocVao::to_string(),
            FromWasmAllocTextureImage2D::to_string(),
            FromWasmBeginRenderTexture::to_string(),
            FromWasmBeginRenderCanvas::to_string(),
            FromWasmSetDefaultDepthAndBlendMode::to_string(),
            FromWasmDrawCall::to_string(),
            
            FromWasmStartMidiInput::to_string(),
            FromWasmSpawnAudioOutput::to_string(),            
        ]);
    }
    
    fn spawn_thread<F>(&mut self, f: F) where F: FnOnce() + Send + 'static {
        let closure_box: Box<dyn FnOnce() + Send + 'static> = Box::new(f);
        let closure_ptr = Box::into_raw(Box::new(closure_box));
        self.os.from_wasm(FromWasmCreateThread {closure_ptr: closure_ptr as u32});
    }
    
    fn web_socket_open(&mut self, url: String, rec: WebSocketAutoReconnect) -> WebSocket {
        let web_socket_id = self.web_socket_id;
        self.web_socket_id += 1;
        
        self.os.from_wasm(FromWasmWebSocketOpen {
            url,
            web_socket_id: web_socket_id as usize,
            auto_reconnect: if let WebSocketAutoReconnect::Yes = rec {true} else {false},
            
        });
        WebSocket(web_socket_id)
    }
    
    fn web_socket_send(&mut self, websocket: WebSocket, data: Vec<u8>) {
        self.os.from_wasm(FromWasmWebSocketSend {
            web_socket_id: websocket.0 as usize,
            data: WasmDataU8::from_vec_u8(data)
        });
    }
    /*
    fn start_midi_input(&mut self) {
        self.platform.from_wasm(FromWasmStartMidiInput {
        });
    }
    
    fn spawn_audio_output<F>(&mut self, f: F) where F: FnMut(AudioTime, &mut dyn AudioOutputBuffer) + Send + 'static {
        let closure_ptr = Box::into_raw(Box::new(WebAudioOutputClosure {
            callback: Box::new(f),
            output_buffer: WebAudioOutputBuffer::default()
        }));
        self.platform.from_wasm(FromWasmSpawnAudioOutput {closure_ptr: closure_ptr as u32});
    }*/
}

extern "C" {
    pub fn js_post_signal(signal_hi: u32, signal_lo: u32);
}

#[export_name = "wasm_thread_entrypoint"]
#[cfg(target_arch = "wasm32")]
pub unsafe extern "C" fn wasm_thread_entrypoint(closure_ptr: u32) {
    let closure = Box::from_raw(closure_ptr as *mut Box<dyn FnOnce() + Send + 'static>);
    closure();
}

#[export_name = "wasm_thread_alloc_tls_and_stack"]
#[cfg(target_arch = "wasm32")]
pub unsafe extern "C" fn wasm_thread_alloc_tls_and_stack(tls_size: u32) -> u32 {
    let mut v = Vec::<u64>::new();
    v.reserve_exact(tls_size as usize);
    let mut v = std::mem::ManuallyDrop::new(v);
    v.as_mut_ptr() as u32
}

// storage buffers for graphics API related platform
#[derive(Default)]
pub struct CxOs {
    pub (crate) window_geom: WindowGeom,
    
    pub (crate) from_wasm: Option<FromWasmMsg>,
    
    pub (crate) vertex_buffers: usize,
    pub (crate) index_buffers: usize,
    pub (crate) vaos: usize,
    
    pub (crate) xr_last_inputs: Option<Vec<XRInput >>,
    
    pub (crate) web_midi_access: WebMidiAccess,
    
    pub (crate) to_wasm_js: Vec<String>,
    pub (crate) from_wasm_js: Vec<String>
}

impl CxOs {
    
    pub fn append_to_wasm_js(&mut self, strs: &[String]) {
        self.to_wasm_js.extend_from_slice(strs);
    }
    
    pub fn append_from_wasm_js(&mut self, strs: &[String]) {
        self.from_wasm_js.extend_from_slice(strs);
    }
    
    pub fn from_wasm(&mut self, from_wasm: impl FromWasm) {
        self.from_wasm.as_mut().unwrap().from_wasm(from_wasm);
    }
}

#[export_name = "wasm_get_js_message_bridge"]
#[cfg(target_arch = "wasm32")]
pub unsafe extern "C" fn wasm_get_js_message_bridge(cx_ptr: u32) -> u32 {
    let cx = &mut *(cx_ptr as *mut Cx);
    let mut msg = FromWasmMsg::new();
    let mut out = String::new();
    
    out.push_str("return {\n");
    out.push_str("ToWasmMsg:class extends ToWasmMsg{\n");
    for to_wasm in &cx.os.to_wasm_js {
        out.push_str(to_wasm);
    }
    out.push_str("},\n");
    out.push_str("FromWasmMsg:class extends FromWasmMsg{\n");
    for from_wasm in &cx.os.from_wasm_js {
        out.push_str(from_wasm);
    }
    out.push_str("}\n");
    out.push_str("}");
    msg.push_str(&out);
    msg.release_ownership()
}

#[no_mangle]
pub static mut BASE_ADDR: usize = 10;
