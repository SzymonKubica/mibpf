use std::{process::Command, str::FromStr, time::Duration};

use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::Deserialize;

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/admin-tools-website.css"/>

        // sets the document title
        <Title text="µBPF Admin Tools"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes>
                    <Route path="" view=WeatherStation/>
                    <Route path="/admin" view=AdminPage/>
                    <Route path="/*" view=NotFound/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn WeatherStation() -> impl IntoView {
    view! {
        <h1>"Weather Station"</h1>
        <img src="/assets/example-application.png" width="600px" alt="Weather Station"/>
        <ApplicationDeploy/>
        <ApplicationStart/>

    }
}

#[component]
fn AdminPage() -> impl IntoView {
    view! {
        <h1>"µBPF Admin Tools"</h1>
        <DeployForm/>
        <ExecuteForm/>
    }
}

/// 404 - Not Found
#[component]
fn NotFound() -> impl IntoView {
    // set an HTTP status code 404
    // this is feature gated because it can only be done during
    // initial server-side rendering
    // if you navigate to the 404 page subsequently, the status
    // code will not be set because there is not a new HTTP request
    // to the server
    #[cfg(feature = "ssr")]
    {
        // this can be done inline because it's synchronous
        // if it were async, we'd use a server function
        let resp = expect_context::<leptos_actix::ResponseOptions>();
        resp.set_status(actix_web::http::StatusCode::NOT_FOUND);
    }

    view! { <h1>"Not Found"</h1> }
}

#[component]
fn ExecuteForm() -> impl IntoView {
    let (slot, set_slot) = create_signal(0);
    let (response, set_response) = create_signal("Loading".to_string());
    let (target_vm, set_target_vm) = create_signal("rBPF".to_string());
    let (binary_layout, set_binary_layout) = create_signal("RawObjectFile".to_string());
    let (execution_model, set_execution_model) = create_signal("LongRunning".to_string());
    let (use_jit, set_use_jit) = create_signal(false);
    let (jit_compile, set_jit_compile) = create_signal(false);
    let (benchmark, set_benchmark) = create_signal(false);

    let send_execution_request = create_action(|input: &(String, String, usize, String, bool, bool, bool)| {
        let (target_vm, binary_layout, storage_slot, execution_model, use_jit, jit_compile, benchmark) = input.to_owned();
        async move {
            let response = execute(target_vm, binary_layout, storage_slot, execution_model, use_jit, jit_compile, benchmark).await;
            response.unwrap()
        }
    });

    view! {
        <p>"Execution request form"</p>
        <div>
            <input
                type="text"
                on:input=move |ev| {
                    set_slot(event_target_value(&ev).parse::<i32>().unwrap());
                }

                prop:value=slot
            />
            <text>"< SUIT storage slot"</text>
        </div>
        <div>
            <TargetVMSelector target_vm set_target_vm/>
            <text>"< Target VM"</text>
        </div>
        <div>
            <BinaryLayoutSelector binary_layout set_binary_layout/>
            <text>"< Binary format"</text>
        </div>
        <div>
            <ExecutionModelSelector execution_model set_execution_model/>
            <text>"< Binary format"</text>
        </div>
        <div>
            <input
                type="checkbox"
                on:input=move |_| { set_use_jit(!use_jit.get()) }

                prop:checked=use_jit
            />
            <text>"Use JIT"</text>
        </div>
        <div>
            <input
                type="checkbox"
                on:input=move |_| { set_jit_compile(!jit_compile.get()) }

                prop:checked=jit_compile
            />
            <text>"JIT Recompile"</text>
        </div>
        <div>
            <input
                type="checkbox"
                on:input=move |_| { set_benchmark(!benchmark.get()) }

                prop:checked=benchmark
            />
            <text>"Benchmark"</text>
        </div>

        <button on:click=move |_| {
            let _ = send_execution_request
                .dispatch((
                    target_vm.get(),
                    binary_layout.get(),
                    slot.get() as usize,
                    execution_model.get(),
                    use_jit.get(),
                    jit_compile.get(),
                    benchmark.get(),
                ));
            set_response(send_execution_request.value().get().unwrap());
        }>"Execute"</button>
        <p>"Response:"</p>
        <p>
            {move || match send_execution_request.value().get() {
                Some(v) => v,
                None => "Loading".to_string(),
            }}

        </p>
    }
}

#[component]
fn ApplicationDeploy() -> impl IntoView {
    let deploy_application = create_action(|input: &()| {
        async move {
            let _ = bootstrap_application().await;
        }
    });

    view! {
        <div>
            <button on:click=move |_| {
                deploy_application
                    .dispatch(());
            }>"Deploy"</button>
            <text>" Sensor Station"</text>
        </div>
    }
}

#[component]
fn ApplicationStart() -> impl IntoView {

    #[derive(Debug, Deserialize, Clone)]
    struct TemperatureHumidity {
        pub temperature: f32,
        pub humidity: f32,
    }

    #[derive(Debug, Deserialize, Clone)]
    struct SoundLightIntensity {
        pub sound_volume: u32,
        pub light_intensity: u32,
    }

    #[derive(Debug, Deserialize, Clone)]
    struct RunningVMsResponse {
        pub vm_status: [bool; 4]
    }

    let start_application = create_action(|input: &()| {
        async move {
            let response1 = execute("rBPF".to_string(), "RawObjectFile".to_string(), 3, "WithAccessToCoapPacket".to_string(), false, false, false).await;
            let response2 = execute("rBPF".to_string(), "RawObjectFile".to_string(), 4, "WithAccessToCoapPacket".to_string(), false, false, false).await;
            let running_vms = get_running_vms().await;

            logging::log!("Response1: {:?}", response1);
            logging::log!("Response2: {:?}", response2);

            let temperature_humidity: TemperatureHumidity = serde_json::from_str(&response1.unwrap().trim_matches('\0')).unwrap();
            let sound_light: SoundLightIntensity = serde_json::from_str(&response2.unwrap().trim_matches('\0')).unwrap();

            return (temperature_humidity, sound_light, running_vms.unwrap())
        }
    });

    view! {
        <div>
            <button on:click=move |_| {
                start_application
                    .dispatch(());
            }>"Refresh"</button>
            <text>" Collected Data"</text>
        </div>
        <p> Sensor Data </p>
        <div>
            <text>"Temperature: "</text>
            <text>
                {move || match start_application.value().get() {
                    Some(v) => format!("{:.2}°C", v.0.temperature),
                    None => "Pending...".to_string(),
                }}
            </text>
        </div>
        <div>
            <text>"Humidity: "</text>
            <text>
                {move || match start_application.value().get() {
                    Some(v) => format!("{:.2}%", v.0.humidity),
                    None => "Pending...".to_string(),
                }}
            </text>
        </div>
        <div>
            <text>"Sound Volume: "</text>
            <text>
                {move || match start_application.value().get() {
                    Some(v) => format!("{}dB", v.1.sound_volume),
                    None => "Pending...".to_string(),
                }}
            </text>
        </div>
        <div>
            <text>"Light Intensity: "</text>
            <text>
                {move || match start_application.value().get() {
                    Some(v) => format!("{}%", v.1.light_intensity),
                    None => "Pending...".to_string(),
                }}
            </text>
        </div>

        <p> Application Compartment Status </p>
        <div>
            <text>
                {move || match start_application.value().get() {
                    Some(v) => if v.2[3] { "Running" } else {"Crashed"}
                    None => "Pending...",
                }}
            </text>
            <text>" <- LCD Display Update Application"</text>
        </div>
        <div>
            <text>
                {move || match start_application.value().get() {
                    Some(v) => if v.2[2] { "Running" } else {"Crashed"}
                    None => "Pending...",
                }}
            </text>
            <text>" <- Sound & Light Intensity Measurement"</text>
        </div>
        <div>
            <text>
                {move || match start_application.value().get() {
                    Some(v) => if v.2[1] { "Running" } else {"Crashed"}
                    None => "Pending...",
                }}
            </text>
            <text>" <- Temperature & Humidity Intensity Measurement"</text>
        </div>
    }
}



#[component]
fn DeployForm() -> impl IntoView {
    let (name, set_name) = create_signal("display-update-thread.c".to_string());
    let (slot, set_slot) = create_signal(0);
    let (target_vm, set_target_vm) = create_signal("rBPF".to_string());
    let (binary_layout, set_binary_layout) = create_signal("RawObjectFile".to_string());


    let send_deploy_request = create_action(|input: &(String, String, String, usize)| {
        let (source_file, target_vm, binary_layout, storage_slot) = input.to_owned();
        async move {
            let _ = deploy(source_file, target_vm, binary_layout, storage_slot).await;
        }
    });

    view! {
        <p>"Deployment request form"</p>
        <div>
            <input
                type="text"
                on:input=move |ev| {
                    set_name(event_target_value(&ev));
                }

                // the `prop:` syntax lets you update a DOM property,
                // rather than an attribute.
                prop:value=name
            />
            <text>"< File name"</text>
        </div>
        <div>
            <input
                type="text"
                on:input=move |ev| {
                    set_slot(event_target_value(&ev).parse::<i32>().unwrap());
                }

                prop:value=slot
            />
            <text>"< SUIT storage slot"</text>
        </div>
        <div>
            <TargetVMSelector target_vm set_target_vm/>
            <text>"< Target VM"</text>
        </div>
        <div>
            <BinaryLayoutSelector binary_layout set_binary_layout/>
            <text>"< Binary format"</text>
        </div>

        <button on:click=move |_| {
            send_deploy_request
                .dispatch((name.get(), target_vm.get(), binary_layout.get(), slot.get() as usize));
        }>"Deploy"</button>
    }
}

#[component]
pub fn TargetVMSelector(target_vm: ReadSignal<String>, set_target_vm: WriteSignal<String>) -> impl IntoView {
    view! {
        <select on:change=move |ev| {
            let new_value = event_target_value(&ev);
            set_target_vm(new_value);
        }>
            <SelectOption value=target_vm is="rBPF"/>
            <SelectOption value=target_vm is="FemtoContainer"/>
        </select>
    }
}


#[component]
pub fn BinaryLayoutSelector(binary_layout: ReadSignal<String>, set_binary_layout: WriteSignal<String>) -> impl IntoView {
    view! {
        <select on:change=move |ev| {
            let new_value = event_target_value(&ev);
            set_binary_layout(new_value);
        }>
            <SelectOption value=binary_layout is="OnlyTextSection"/>
            <SelectOption value=binary_layout is="FemtoContainersHeader"/>
            <SelectOption value=binary_layout is="ExtendedHeader"/>
            <SelectOption value=binary_layout is="RawObjectFile"/>
        </select>
    }
}

#[component]
pub fn ExecutionModelSelector(execution_model: ReadSignal<String>, set_execution_model: WriteSignal<String>) -> impl IntoView {
    view! {
        <select on:change=move |ev| {
            let new_value = event_target_value(&ev);
            set_execution_model(new_value);
        }>
            <SelectOption value=execution_model is="ShortLived"/>
            <SelectOption value=execution_model is="WithAccessToCoapPacket"/>
            <SelectOption value=execution_model is="LongRunning"/>
        </select>
    }
}

#[component]
pub fn SelectOption(is: &'static str, value: ReadSignal<String>) -> impl IntoView {
    view! {
        <option value=is selected=move || value() == is>
            {is}
        </option>
    }
}


#[server(DeployRequest, "/deploy")]
pub async fn deploy(source_file: String, target_vm: String, binary_layout: String, storage_slot: usize) -> Result<(), ServerFnError> {
    use micro_bpf_common::{BinaryFileLayout, TargetVM};
    use micro_bpf_common::*;
    use micro_bpf_tools::*;
    let environment: Environment = load_env();

    println!("Env: {:?}", environment);
    println!("Source file: {}", source_file);
    println!("Target VM: {}", target_vm);
    println!("Binary file layout: {}", binary_layout);
    println!("Storage slot: {}", storage_slot);
    let deploy_response = deploy(
        &format!("{}/{}", &environment.src_dir, source_file),
        &environment.out_dir,
        TargetVM::from_str(&target_vm).unwrap(),
        BinaryFileLayout::from_str(&binary_layout).unwrap(),
        &environment.coap_root_dir,
        storage_slot,
        &environment.riot_instance_net_if,
        &environment.riot_instance_ip,
        &environment.host_net_if,
        &environment.host_ip,
        &environment.board_name,
        Some(&environment.micro_bpf_root_dir),
        vec![],
        HelperAccessVerification::PreFlight,
        HelperAccessListSource::ExecuteRequest,
        true,
    )
    .await;

    Ok(deploy_response.unwrap())
}

#[server(RunningVMsRequest, "/get_running_vms")]
pub async fn get_running_vms() -> Result<[bool; 4], ServerFnError> {
    use micro_bpf_tools::*;
    let environment: Environment = load_env();

    let base_url = format!("coap://[{}%{}]/running_vm", environment.riot_instance_ip, environment.host_net_if);

    let response = String::from_utf8(Command::new("aiocoap-client")
        .arg("-m")
        .arg("GET")
        .arg(base_url.clone())
        .output().unwrap().stdout).unwrap();

    Ok(serde_json::from_str(&response).unwrap())
}

#[server(ExecuteRequest, "/execute")]
pub async fn execute(target_vm: String, binary_layout: String, storage_slot: usize, execution_model: String, use_jit: bool, jit_compile: bool, benchmark: bool) -> Result<String, ServerFnError> {
    use micro_bpf_common::*;
    use micro_bpf_tools::*;
    let environment: Environment = load_env();

    println!("Env: {:?}", environment);
    println!("Target VM: {}", target_vm);
    println!("Binary file layout: {}", binary_layout);
    println!("Storage slot: {}", storage_slot);
    println!("Execution model: {}", execution_model);
    println!("Use JIT: {}", use_jit);
    println!("JIT recompile: {}", jit_compile);
    println!("Benchmark: {}", benchmark);

    let execution_response = execute(
        &environment.riot_instance_ip,
        TargetVM::from_str(&target_vm).unwrap(),
        BinaryFileLayout::from_str(&binary_layout).unwrap(),
        storage_slot,
        &environment.host_net_if,
        ExecutionModel::from_str(&execution_model).unwrap(),
        HelperAccessVerification::PreFlight,
        HelperAccessListSource::ExecuteRequest,
        &vec![],
        use_jit,
        jit_compile,
        benchmark
    )
    .await;
    Ok(execution_response.unwrap())
}

#[server(BootstrapApplication, "/weather-station-deploy")]
pub async fn bootstrap_application() -> Result<String, ServerFnError> {
    use micro_bpf_common::*;
    use micro_bpf_tools::*;
    let environment: Environment = load_env();

    let target_vm = TargetVM::Rbpf;
    let binary_layout = BinaryFileLayout::RawObjectFile;

    let application_source = vec![
        "display-update-thread-bug.c", "sound-light-intensity-update-thread.c", "temperature-humidity-update-thread.c", "gcoap_temperature_humidity.c", "gcoap_sound_light_intensity.c"
    ];

    for (i, file) in application_source.iter().enumerate() {
        let deploy_response = deploy(
            &format!("{}/{}", &environment.src_dir, file),
            &environment.out_dir,
            target_vm,
            binary_layout,
            &environment.coap_root_dir,
            i,
            &environment.riot_instance_net_if,
            &environment.riot_instance_ip,
            &environment.host_net_if,
            &environment.host_ip,
            &environment.board_name,
            Some(&environment.micro_bpf_root_dir),
            vec![],
            HelperAccessVerification::PreFlight,
            HelperAccessListSource::ExecuteRequest,
            true,
        )
        .await;

        match i {
            0 => {
              println!("Deploying display update thread...");
              // This file is large and takes long to deploy.
            std::thread::sleep(Duration::from_secs(2));
            },
            1 => {
              println!("Deploying sound and light intensity update thread...");
              std::thread::sleep(Duration::from_secs(1));
            },
            2 => {
              println!("Deploying temperature and humidity update thread...");
              std::thread::sleep(Duration::from_secs(1));
            },
            _ => {
              println!("Deploying query script...");
              std::thread::sleep(Duration::from_secs(1));
            }
        };
    }

    // Now we send execution requests (only the long-running programs are started)
    for (i, file) in application_source.iter().take(3).enumerate() {
        println!("Executing program: {}...", file);
        let execution_response = execute(
            &environment.riot_instance_ip,
            target_vm,
            binary_layout,
            i,
            &environment.host_net_if,
            ExecutionModel::LongRunning,
            HelperAccessVerification::PreFlight,
            HelperAccessListSource::ExecuteRequest,
            &vec![],
            false,
            false,
            false
        )
        .await;
        if let Ok(r) = execution_response {
            println!("Execution response: {}", r);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    Ok("".to_string())

}
