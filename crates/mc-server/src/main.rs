mod app;
mod config;
mod context;
mod metrics;
mod tick;

// jemalloc: 比系统默认分配器减少 15-25% 内存碎片 (Linux/RPi5 专属)
#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use mc_network::connection;
use mc_network::listener::ServerListener;
use mc_player::mob::MobAiState;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Logging: console (text) + file (daily rotation) ──
    let log_dir = std::path::Path::new("logs");
    let _ = std::fs::create_dir_all(log_dir);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let log_date = format!("{}", now.as_secs() / 86400); // day number since epoch
    let log_path = log_dir.join(format!("server-{}.log", log_date));
    let log_file = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(&log_path)
        .expect("Failed to open log file");
    let (_log_tx, log_rx) = std::sync::mpsc::sync_channel::<String>(1024);
    // Background log writer thread
    std::thread::spawn(move || {
        use std::io::Write;
        let mut file = log_file;
        for msg in log_rx {
            let _ = writeln!(file, "{}", msg);
            let _ = file.flush();
        }
    });
    // Console subscriber
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    info!("Logging to {}", log_path.display());

    // Parse CLI args
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 {
        match args[1].as_str() {
            "--help" | "-h" => {
                println!("Minecraft LAN Server — Rust Edition");
                println!();
                println!("Usage:  mc-server");
                println!();
                println!("Config:  config/default.toml");
                println!("Env:     MCS_SECTION__KEY=value (e.g. MCS_SERVER__PORT=25566)");
                return Ok(());
            }
            other => {
                eprintln!("Unknown option: {} (use --help for usage)", other);
                return Ok(());
            }
        }
    }

    // Resolve server root directory (executable location, not CWD)
    let server_root = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    info!("Server root: {}", server_root.display());

    // Config path: MCS_CONFIG env → executable dir → CWD fallback
    let config_path = std::env::var("MCS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| server_root.join("config/default.toml"));

    // ── jemalloc configuration ──
    #[cfg(target_os = "linux")]
    {
        // Report jemalloc status — MALLOC_CONF must be set via environment before launch.
        // Use scripts/setup-rpi.sh for optimal RPi 5 configuration:
        //   MALLOC_CONF="background_thread:true,dirty_decay_ms:5000,muzzy_decay_ms:5000,narenas:4,lg_tcache_max:16,metadata_thp:always"
        let has_conf = std::env::var("MALLOC_CONF").ok();
        match has_conf {
            Some(ref conf) if !conf.is_empty() => info!("jemalloc: active (MALLOC_CONF={})", conf),
            _ => info!("jemalloc: active (no MALLOC_CONF set — consider using scripts/setup-rpi.sh for optimal RPi 5 tuning)"),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        info!("jemalloc: not active (Linux-only)");
    }

    info!("Loading config from: {}", config_path.display());
    let config = config::Config::load(&config_path)?;
    let mut app = app::App::new(config);

    let gen_name = app.config.world.generator.clone();
    if gen_name != "flat"
        && let Err(e) = app.world.generators.set_active(&gen_name) {
            warn!("{}", e);
        }
    let world_seed = app.world.seed;
    let cached_generator = app.world.generators.active().clone();
    let gen_active = app.world.generators.active().name().to_string();
    info!("Terrain generator: '{}'", gen_active);

    let player_manager = Arc::new(mc_player::player::PlayerManager::new());
    let next_entity_id = Arc::new(std::sync::atomic::AtomicI32::new(10000));
    let mut mob_mgr = mc_player::mob::MobManager::new();
    mob_mgr.next_entity_id = next_entity_id.clone();
    let mob_manager = Arc::new(mob_mgr);
    let villager_data: Arc<dashmap::DashMap<i32, mc_player::villager::VillagerData>> = Arc::new(dashmap::DashMap::new());
    let container_manager = Arc::new(mc_player::container::ContainerManager::new());
    // ── Recipe registry + datapack loading ──
    let mut recipe_registry = mc_player::recipe::RecipeRegistry::new();
    let datapacks_dir = server_root.join("datapacks");
    let mut datapack_loader = mc_plugin::datapack::DatapackLoader::new(&datapacks_dir);
    let _ = datapack_loader.load_pack("vanilla"); // built-in vanilla extensions
    // Register parsed recipes (fix: previously discarded with `let _ = recipe`)
    let datapack_recipes = datapack_loader.load_all_recipes();
    let dp_recipe_count = datapack_recipes.len();
    for recipe in datapack_recipes {
        recipe_registry.register(recipe);
    }
    if dp_recipe_count > 0 {
        info!("Registered {} recipes from datapacks", dp_recipe_count);
    }
    let recipe_registry = Arc::new(recipe_registry);

    let fishing_manager = Arc::new(parking_lot::RwLock::new(mc_player::fishing::FishingManager::new()));
    let brewing_manager = Arc::new(parking_lot::RwLock::new(mc_player::brewing::BrewingStandManager::new()));
    let beacon_manager = Arc::new(parking_lot::RwLock::new(mc_player::beacon::BeaconManager::new()));
    let furnace_manager = Arc::new(parking_lot::RwLock::new(mc_player::furnace::FurnaceManager::new()));
    let advancement_tracker = Arc::new(parking_lot::RwLock::new(mc_player::advancement::AdvancementTracker::new()));
    let advancement_registry = Arc::new(mc_player::advancement::AdvancementRegistry::new());
    let brewing_registry = Arc::new(mc_player::brewing::BrewingRegistry::new());
    let raid_manager = Arc::new(mc_player::raid::RaidManager::new());
    let _stat_tracker = Arc::new(mc_player::statistics::StatTracker::new());
    let redstone_engine = Arc::new(mc_world::redstone::RedstoneEngine::new());
    // Register container fill provider for redstone comparators (fixes constant 0.5)
    {
        let cm = container_manager.clone();
        mc_world::redstone::set_container_fill_provider(Arc::new(move |x, y, z| {
            let slots = cm.get_persistent((x, y, z));
            if slots.is_empty() { return 0.0; }
            let filled = slots.iter().filter(|s| s.is_some()).count();
            filled as f32 / slots.len() as f32
        }));
    }
    // Register entity-on-block provider for pressure plates & tripwire hooks
    {
        let pm = player_manager.clone();
        let mm = mob_manager.clone();
        mc_world::redstone::set_entity_on_block_provider(Arc::new(move |x, y, z| {
            let mut count = 0u8;
            // Check players standing on this block
            for p in pm.all_players() {
                let px = p.position.x.floor() as i32;
                let py = (p.position.y - 0.01).floor() as i32; // foot position
                let pz = p.position.z.floor() as i32;
                if px == x && py == y && pz == z {
                    count += 1;
                }
            }
            // Check mobs standing on this block
            for mob in mm.all_mobs() {
                let mx = mob.position.x.floor() as i32;
                let my = (mob.position.y - 0.01).floor() as i32;
                let mz = mob.position.z.floor() as i32;
                if mx == x && my == y && mz == z {
                    count += 1;
                }
            }
            count.min(100) // cap at reasonable max
        }));
    }
    let fluid_engine = Arc::new(mc_world::fluid::FluidEngine::new());
    let chunk_store = mc_world::chunk_store::ChunkStore::new();
    let (shutdown_tx, _shutdown_rx) = broadcast::channel::<()>(1);
    let (save_trigger_tx, _) = broadcast::channel::<()>(1);

    // Connection semaphore: limit concurrent connections (RPi5: 50 max)
    let conn_semaphore = Arc::new(tokio::sync::Semaphore::new(50));

    let world_state = Arc::new(parking_lot::RwLock::new(
        mc_core::world_state::WorldState::new(world_seed),
    ));




    let save_manager = Rc::new(
        mc_persistence::SaveManager::new(
            &server_root.join("data"),
            &app.config.persistence.player_db,
        )
        .map_err(|e| {
            error!("{}", e);
            std::process::exit(1);
        })
        .unwrap()
    );

    // Configure chunk compression
    mc_world::chunk_store::set_compression(
        mc_world::chunk_store::ChunkCompression::from_str(&app.config.persistence.chunk_compression)
    );
    info!("Chunk compression: {:?}", mc_world::chunk_store::compression());

    // Load persistent state from DB → PlayerManager
    let banned = save_manager.load_banned_uuids();
    for uuid in &banned {
        player_manager.ban(*uuid);
    }
    info!("Loaded {} bans from database", banned.len());
    let whitelisted = save_manager.load_whitelist_uuids();
    for uuid in &whitelisted {
        player_manager.add_whitelist(*uuid);
    }
    info!("Loaded {} whitelist entries from database", whitelisted.len());

    // Pre-load all player data for login-time restoration
    let saved_player_data = Arc::new(parking_lot::RwLock::new(
        save_manager.load_all_player_data()
    ));
    info!("Loaded {} player records from database", saved_player_data.read().len());

    // Load container contents from disk
    let containers_path = server_root.join("data").join("containers.bin");
    if containers_path.exists() {
        match std::fs::read(&containers_path) {
            Ok(data) => {
                container_manager.deserialize_all(&data);
                info!("Loaded container data from {}", containers_path.display());
            }
            Err(e) => tracing::warn!("Failed to load container data: {}", e),
        }
    }

    let world_arc = Arc::new(parking_lot::RwLock::new(app.world));

    let ctx = context::ServerContext::new(
        player_manager.clone(),
        shutdown_tx.clone(),
        world_state.clone(),
        app.config.server.motd.clone(),
        app.config.server.max_players,
    );

    if app.config.admin.console_enabled {
        let console = mc_admin::console::ConsoleInput::new(
            ctx.command_dispatcher.clone(),
            player_manager.clone(),
            shutdown_tx.clone(),
            world_state.clone(),
        );
        tokio::spawn(async move { console.run().await });
    }

    // RCON
    if app.config.admin.rcon_enabled {
        let rcon = mc_admin::rcon::RconServer::new(
            &app.config.server.host,
            app.config.admin.rcon_port,
            &app.config.admin.rcon_password,
            ctx.command_dispatcher.clone(),
            player_manager.clone(),
            shutdown_tx.clone(),
            world_state.clone(),
        );
        tokio::spawn(async move { rcon.run().await });
    }

    let listener = ServerListener::bind(&app.config.server.host, app.config.server.port).await?;

    if app.config.lan.enabled {
        match mc_network::lan_broadcast::LanBroadcaster::new(
            &app.config.server.motd,
            app.config.server.port,
            &app.config.lan.multicast_group,
            app.config.lan.broadcast_interval_ms,
        )
        .await
        {
            Ok(b) => {
                info!("LAN broadcast enabled");
                tokio::spawn(async move { b.run().await });
            }
            Err(e) => warn!("Failed to start LAN broadcast: {}", e),
        }
    }

    // Start Prometheus metrics server if enabled
    if app.config.metrics.prometheus_enabled {
        let metrics_bind = format!("0.0.0.0:{}", app.config.metrics.prometheus_port);
        let metrics_tick = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let metrics_pm = player_manager.clone();
        let metrics_cs = chunk_store.clone();
        let metrics_start = std::time::Instant::now();
        let metrics_tc = metrics_tick.clone();
        let m_bind = metrics_bind.clone();
        tokio::spawn(async move {
            metrics::serve_metrics(&m_bind, metrics_pm, metrics_cs, metrics_start, metrics_tc).await;
        });
        info!("Prometheus metrics enabled on {}", metrics_bind);
    }

    let world_dir = server_root.join("data/world/region");

    // ── Plugin system initialization ──
    let plugin_ctx = mc_plugin::plugin::PluginContext {
        player_manager: player_manager.clone(),
        command_dispatcher: ctx.command_dispatcher.clone(),
        world_state: world_state.clone(),
        chunk_store: chunk_store.clone(),
        mob_manager: mob_manager.clone(),
        container_manager: container_manager.clone(),
        shutdown_tx: shutdown_tx.clone(),
        data_dir: std::path::PathBuf::from("plugins"),
    };
    let plugin_manager = std::sync::Arc::new(mc_plugin::plugin::PluginManager::new());

    // Register built-in CorePlugin — demonstrates plugin lifecycle is wired
    struct CorePlugin { enabled: bool }
    impl mc_plugin::plugin::NativePlugin for CorePlugin {
        fn name(&self) -> &str { "Core" }
        fn version(&self) -> &str { "1.0.0" }
        fn author(&self) -> &str { "mc-server" }
        fn on_enable(&mut self, _ctx: &mc_plugin::plugin::PluginContext) {
            tracing::info!("CorePlugin enabled — plugin lifecycle is active");
        }
        fn on_tick(&mut self, ctx: &mc_plugin::plugin::PluginContext, tick: u64) {
            if tick.is_multiple_of(1200) { // every 60 seconds at 20 TPS
                tracing::debug!("CorePlugin tick {}: {} players online", tick, ctx.player_manager.online_count());
            }
        }
        fn on_player_join(&mut self, _ctx: &mc_plugin::plugin::PluginContext, _uuid: &uuid::Uuid, username: &str) {
            tracing::info!("CorePlugin: {} joined the server", username);
        }
        fn on_player_leave(&mut self, _ctx: &mc_plugin::plugin::PluginContext, _uuid: &uuid::Uuid) {
            tracing::info!("CorePlugin: player left");
        }
        fn on_disable(&mut self) {
            tracing::info!("CorePlugin disabled");
        }
        fn is_enabled(&self) -> bool { self.enabled }
        fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    }
    plugin_manager.register(Box::new(CorePlugin { enabled: true }));
    plugin_manager.enable_all(&plugin_ctx);
    info!("Plugin system initialized with CorePlugin");

    let server_ref = connection::ServerRef {
        motd: app.config.server.motd.clone(),
        max_players: app.config.server.max_players,
        protocol_version: app.config.server.protocol_version,
        version_name: app.config.server.version_name.clone(),
        online_mode: app.config.server.online_mode,
        compression_threshold: app.config.server.compression_threshold,
        world_seed,
        generator_name: app.config.world.generator.clone(),
        view_distance: app.config.world.view_distance,
        max_view_distance: app.config.world.view_distance,
        player_manager: player_manager.clone(),
        command_dispatcher: ctx.command_dispatcher.clone(),
        shutdown_tx: shutdown_tx.clone(),
        chunk_store: chunk_store.clone(),
        world_state: world_state.clone(),
        world_dir: world_dir.clone(),
        saved_player_data: saved_player_data.clone(),
        next_entity_id: next_entity_id.clone(),
        save_trigger: save_trigger_tx.clone(),
        generator: cached_generator.clone(),
        mob_manager: mob_manager.clone(),
        container_manager: container_manager.clone(),
        raid_manager: raid_manager.clone(),
        recipe_registry: recipe_registry.clone(),
        fishing_manager: fishing_manager.clone(),
        brewing_manager: brewing_manager.clone(),
        beacon_manager: beacon_manager.clone(),
        advancement_tracker: advancement_tracker.clone(),
        advancement_registry: advancement_registry.clone(),
        dirty_chunks_broadcast: std::sync::Arc::new(parking_lot::RwLock::new(std::collections::HashSet::new())),
        dirty_blocks: std::sync::Arc::new(mc_network::connection::DirtyBlockTracker::new()),
        server_links: app.config.server.server_links.iter()
            .map(|l| (l.label.clone(), l.url.clone()))
            .collect(),
        dropped_items: std::sync::Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
        jukebox_discs: std::sync::Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
        furnace_manager: furnace_manager.clone(),
        plugin_manager: plugin_manager.clone(),
        plugin_ctx: plugin_ctx.clone(),
    };
    let gen_for_preload = cached_generator.clone();

    let next_entity_id_for_tick = next_entity_id.clone();
    let dropped_for_tick = server_ref.dropped_items.clone();

    info!("Server ready! Connect via localhost:{}", app.config.server.port);
    info!("Protocol: {} (v{}) | Online: {} | Generator: {}",
        app.config.server.version_name, app.config.server.protocol_version,
        if app.config.server.online_mode { "yes" } else { "no" }, gen_active);

    // Preload spawn chunks in parallel (Rayon)
    {
        let gen_ref: &dyn mc_world::generator::TerrainGenerator = gen_for_preload.as_ref();
        let ws = world_state.read();
        let spawn_cx = (ws.spawn_x as i32) >> 4;
        let spawn_cz = (ws.spawn_z as i32) >> 4;
        let ws_seed = ws.seed;
        drop(ws);
        info!("Preloading spawn chunks around ({}, {}) with {} threads...", spawn_cx, spawn_cz, rayon::current_num_threads());
        chunk_store.preload_spawn(spawn_cx, spawn_cz, 8, gen_ref, ws_seed);
        info!("Spawn preload complete — {} chunks in memory", chunk_store.count());
    }

    // ── Spawn subsystem tasks ──

    // Ctrl+C / SIGTERM → graceful shutdown
    let shutdown_tx_ctrl = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutdown signal received, saving...");
        let _ = shutdown_tx_ctrl.send(());
    });
    #[cfg(unix)]
    {
        let shutdown_tx_term = shutdown_tx.clone();
        tokio::spawn(async move {
            if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                sig.recv().await;
            }
            info!("SIGTERM received, shutting down gracefully...");
            let _ = shutdown_tx_term.send(());
        });
    }

    // Extract handles before server_ref is moved into accept loop
    let dirty_blocks_for_tick = server_ref.dirty_blocks.clone();
    let dirty_chunks_for_tick = server_ref.dirty_chunks_broadcast.clone();

    // Accept loop (spawned — TcpListener is Send, with connection limit)
    let accept_shutdown = shutdown_tx.subscribe();
    let accept_sem = conn_semaphore.clone();
    tokio::spawn(async move {
        accept_loop(listener, server_ref, accept_shutdown, accept_sem).await;
    });

    // systemd watchdog — notify every 15s (Linux only)
    #[cfg(target_os = "linux")]
    tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            // Write to NOTIFY_SOCKET for systemd watchdog
            if let Ok(sock_path) = std::env::var("NOTIFY_SOCKET") {
                use std::os::unix::net::UnixDatagram;
                if let Ok(sock) = UnixDatagram::unbound() {
                    let _ = sock.send_to(b"WATCHDOG=1", &sock_path);
                }
            }
        }
    });

    // Game tick loop — runs on main thread
    let tick_rate = app.config.performance.tick_rate.clamp(1, 1000);
    let tick_interval_ms: u64 = (1000 / tick_rate) as u64;
    let save_interval_ticks = app.config.persistence.save_interval_ticks;
    let mut shutdown_rx_for_tick = shutdown_tx.subscribe();
    let mut save_trigger_rx = save_trigger_tx.subscribe();

    // Capture handle for async I/O offloading
    let io_handle = tokio::runtime::Handle::current();

    let mut tick_interval = tokio::time::interval(tokio::time::Duration::from_millis(tick_interval_ms));
    tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut tick_count: u64 = 0;

    loop {
        tokio::select! {
            _ = tick_interval.tick() => {
                let tick_start = std::time::Instant::now();
                // Check tick control
                {
                    let ws = world_state.read();
                    if ws.tick_frozen { continue; }
                    if ws.tick_sprint_rate > 0 {
                        let new_interval = 1000 / ws.tick_sprint_rate as u64;
                        if tick_interval.period().as_millis() as u64 != new_interval {
                            drop(ws);
                            tick_interval = tokio::time::interval(tokio::time::Duration::from_millis(new_interval));
                            tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                        }
                    } else if tick_interval.period().as_millis() as u64 != tick_interval_ms {
                        tick_interval = tokio::time::interval(tokio::time::Duration::from_millis(tick_interval_ms));
                        tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    }
                }
                tick_count = tick_count.wrapping_add(1);

                // 推进世界时间
                world_state.write().add_time(1);

                // Weather cycle (every 20 ticks)
                if tick_count.is_multiple_of(20) {
                    tick::tick_weather(&world_state, &player_manager);
                    plugin_manager.tick_all(&plugin_ctx, tick_count);
                }

                // Tick hunger and effects for all online players
                player_manager.tick_hunger();
                player_manager.tick_effects(tick_count);
                // Poison/Wither periodic damage
                if tick_count.is_multiple_of(25) {
                    for p in player_manager.all_players() {
                        player_manager.tick_effect_damage(&p.uuid);
                    }
                }
                // Environmental damage (void + fire + drowning, every 20 ticks)
                if tick_count.is_multiple_of(20) {
                    tick::tick_environmental_damage(&player_manager, &chunk_store);
                }
                // Tick furnace smelting (every tick for correct vanilla speed)
                furnace_manager.write().tick();
                // Tick redstone engine (every 2 ticks)
                if tick_count.is_multiple_of(2) {
                    redstone_engine.tick(&chunk_store);
                }
                // Hopper item transfer (every 8 ticks ≈ 0.4s)
                if tick_count.is_multiple_of(8) {
                    redstone_engine.tick_hoppers(&chunk_store);
                    // Transfer items between adjacent containers
                    let ready = redstone_engine.get_ready_hoppers();
                    for (hx, hy, hz) in ready {
                        // Pull from above container → push to below container
                        let above = container_manager.find_window_at(hx, hy + 1, hz);
                        let below = container_manager.find_window_at(hx, hy - 1, hz);
                        if let (Some(src), Some(dst)) = (above, below) {
                            // Try to move 1 item from source first non-empty slot to dest
                            for s in 0..27u8 {
                                if let Some(stack) = container_manager.get_slot(src, s as usize) {
                                    // Find empty or matching slot in dest
                                    let mut transferred = false;
                                    for d in 0..27u8 {
                                        let dst_stack = container_manager.get_slot(dst, d as usize);
                                        if dst_stack.is_none() || (dst_stack.as_ref().map(|ds| ds.item.id == stack.item.id && ds.count < ds.max_count).unwrap_or(false)) {
                                            container_manager.set_slot(src, s as usize, None);
                                            let mut new_stack = stack.clone();
                                            if let Some(ds) = dst_stack {
                                                new_stack.count = (ds.count + 1).min(ds.max_count);
                                            } else {
                                                new_stack.count = 1;
                                            }
                                            container_manager.set_slot(dst, d as usize, Some(new_stack));
                                            transferred = true;
                                            break;
                                        }
                                    }
                                    if transferred { break; }
                                }
                            }
                        }
                    }
                }
                // Process TNT explosions (lock-free — drain pending_explosions via internal Mutex)
                {
                    let explosions: Vec<_> = redstone_engine.pending_explosions.lock().drain(..).collect();
                    for (ex, ey, ez, power) in explosions {
                        // Destroy blocks in radius
                        let mut affected_chunks: Vec<mc_core::position::ChunkPos> = Vec::new();
                        for dx in -(power as i32)..=(power as i32) {
                            for dy in -(power as i32)..=(power as i32) {
                                for dz in -(power as i32)..=(power as i32) {
                                    let dist = ((dx*dx + dy*dy + dz*dz) as f32).sqrt();
                                    if dist <= power {
                                        let wx = ex + dx; let wy = ey + dy; let wz = ez + dz;
                                        if (-64..=319).contains(&wy) {
                                            let cp = mc_core::position::ChunkPos::new(wx >> 4, wz >> 4);
                                            if let Some(mut chunk) = chunk_store.get_mut(&cp) {
                                                let block = chunk.get_block((wx & 0xF) as usize, wy, (wz & 0xF) as usize);
                                                if !block.is_air() && block.id != 266 {
                                                    chunk.set_block((wx & 0xF) as usize, wy, (wz & 0xF) as usize, mc_core::block::BlockState::AIR);
                                                    dirty_blocks_for_tick.mark_block(wx, wy, wz, mc_core::block::BlockState::AIR.id);
                                                    if !affected_chunks.contains(&cp) {
                                                        affected_chunks.push(cp);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Mark affected chunks for full rebroadcast to all players
                        for cp in &affected_chunks {
                            dirty_chunks_for_tick.write().insert(*cp);
                        }
                        // Damage nearby players
                        for p in player_manager.all_players() {
                            let dx = p.position.x - ex as f64;
                            let dy = p.position.y - ey as f64;
                            let dz = p.position.z - ez as f64;
                            let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                            if dist <= power as f64 * 2.0 {
                                let dmg = (power as f64 * 7.0 * (1.0 - dist / (power as f64 * 2.0))) as f32;
                                let _ = player_manager.apply_damage(&p.uuid, dmg, tick_count);
                            }
                        }
                    }
                }
                // Minecart rail physics (every 10 ticks)
                if tick_count.is_multiple_of(10) {
                    for eid in mob_manager.all_entity_ids() {
                        if let Some(mob) = mob_manager.get(eid)
                            && matches!(mob.mob_type, 10 | 40 | 41 | 42 | 107) { // minecart variants
                                let mx = mob.position.x as i32;
                                let my = mob.position.y as i32;
                                let mz = mob.position.z as i32;
                                let cp = mc_core::position::ChunkPos::new(mx >> 4, mz >> 4);
                                if let Some(chunk) = chunk_store.get(&cp) {
                                    let rail = chunk.get_block((mx & 0xF) as usize, my, (mz & 0xF) as usize);
                                    let powered = redstone_engine.signal_map
                                        .get(&(mx, my, mz)).map(|v| *v > 0).unwrap_or(false);
                                    // Powered rail (27): accelerate when powered
                                    if rail.id == mc_world::redstone::POWERED_RAIL_ID && powered {
                                        let angle = (mob.position.yaw as f64).to_radians();
                                        let speed = 0.4;
                                        mob_manager.send_position(eid,
                                            mob.position.x + angle.sin() * speed,
                                            mob.position.y,
                                            mob.position.z + angle.cos() * speed);
                                    }
                                    // Detector rail (28): output signal 15 when minecart is on top
                                    if mc_world::redstone::is_detector_rail(rail.id) {
                                        redstone_engine.signal_map.insert((mx, my, mz), 15);
                                    }
                                    // Activator rail (157): eject rider when powered
                                    if mc_world::redstone::is_activator_rail(rail.id) && powered {
                                        // Eject passengers — simplified: bump rider position up
                                        mob_manager.send_position(eid,
                                            mob.position.x,
                                            mob.position.y + 1.5,
                                            mob.position.z);
                                    }
                                }
                            }
                    }
                }

                // Block physics: falling sand/gravel, fire spread, grass spread (every 20 ticks)
                if tick_count.is_multiple_of(20) {
                    mc_world::physics::tick_physics(&chunk_store);
                }
                // Crop growth (every 200 ticks = 10 seconds)
                if tick_count.is_multiple_of(200) {
                    mc_world::crops::tick_crops(&chunk_store);
                }

                // Tick fluid physics (every 5 ticks)
                if tick_count.is_multiple_of(5) {
                    fluid_engine.tick(&chunk_store);
                }
                // Tick brewing stands (every 5 ticks)
                if tick_count.is_multiple_of(5) {
                    brewing_manager.write().tick(&brewing_registry, &container_manager);
                }
                // Tick fishing bobbers (every 20 ticks)
                if tick_count.is_multiple_of(20) {
                    let events = fishing_manager.write().tick();
                    for event in &events {
                        match event {
                            mc_player::fishing::FishingTickEvent::Bite { entity_id: _ } => {
                                // Players detect this via fishing state checks
                            }
                            mc_player::fishing::FishingTickEvent::Expire { entity_id: _ } => {
                                // Bobber cleaned up by manager
                            }
                        }
                    }
                }
                // Tick beacon effects (every 80 ticks = 4 seconds)
                if tick_count.is_multiple_of(80) {
                    beacon_manager.write().tick(&chunk_store, &player_manager);
                }
                // Copper bulb natural oxidation (every 72000 ticks ≈ 1 hour)
                if tick_count.is_multiple_of(72000) {
                    tick::tick_copper_oxidation(&chunk_store, &dirty_chunks_for_tick);
                }
                // Mob spawning
                if tick_count.is_multiple_of(100) {
                    tick::tick_hostile_spawning(&player_manager, &mob_manager, &chunk_store, &world_state, &next_entity_id_for_tick);
                }
                if tick_count.is_multiple_of(200) {
                    tick::tick_passive_spawning(&player_manager, &mob_manager, &chunk_store, &world_state, &next_entity_id_for_tick);
                }
                // Crafter activation (every 20 ticks)
                if tick_count.is_multiple_of(20) {
                    let activations = redstone_engine.take_dispenser_activations();
                    for (x, y, z) in activations {
                        let cp = mc_core::position::ChunkPos::new(x >> 4, z >> 4);
                        if let Some(ch) = chunk_store.get(&cp) {
                            let block = ch.get_block((x & 0xF) as usize, y, (z & 0xF) as usize);
                            // Crafter: auto-craft if powered
                            if block.id == mc_world::redstone::CRAFTER_ID
                                && let Some(container) = container_manager.find_window_at(x, y, z) {
                                    let _slots: Vec<_> = (0..9).filter_map(|i| container_manager.get_slot(container, i)).collect();
                                    // Match recipe from 3x3 grid
                                    let grid: [Option<mc_player::inventory::ItemStack>; 9] = std::array::from_fn(|i| {
                                        container_manager.get_slot(container, i)
                                    });
                                    if let Some((_, _recipe)) = recipe_registry.find_match_3x3(&grid) {
                                        // Consume 1 from each occupied slot and spawn result
                                        for s in 0..9u8 {
                                            if let Some(stack) = container_manager.get_slot(container, s as usize) {
                                                if stack.count <= 1 {
                                                    container_manager.set_slot(container, s as usize, None);
                                                } else {
                                                    let mut reduced = stack.clone();
                                                    reduced.count -= 1;
                                                    container_manager.set_slot(container, s as usize, Some(reduced));
                                                }
                                            }
                                        }
                                        // Spawn result item entity in front
                                        let eid = next_entity_id_for_tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        player_manager.broadcast_mob_spawn(eid, uuid::Uuid::new_v4(), 54, x as f64 + 0.5, y as f64 + 1.0, z as f64 + 0.5);
                                    }
                                }
                        }
                    }
                }
                // Tick mob AI (with player proximity for hostile chase, projectile spawning)
                mob_manager.tick_ai(Some(&player_manager));

                // Tick projectiles — update positions, despawn expired
                // Tick projectiles + detect player hits for bow enchantments (Power/Flame/Punch)
                let proj_events = mob_manager.tick_projectiles();
                for ev in &proj_events {
                    if let mc_player::mob::ProjectileEvent::Despawn(eid) = ev
                        && let Some(proj) = mob_manager.projectiles.get(eid) {
                            let px = proj.position.x; let py = proj.position.y; let pz = proj.position.z;
                            // Handle splash/lingering potion effects
                            if proj.projectile_type == mc_player::mob::ProjectileType::SplashPotion
                                || proj.projectile_type == mc_player::mob::ProjectileType::LingeringPotion {
                                    let radius = if proj.projectile_type == mc_player::mob::ProjectileType::LingeringPotion { 5.0 } else { 8.0 };
                                    // Splash: instant effect application
                                    for player in player_manager.all_players() {
                                        let dx = player.position.x - px;
                                        let dy = (player.position.y + 1.0) - py;
                                        let dz = player.position.z - pz;
                                        let dist_sq = dx*dx + dy*dy + dz*dz;
                                        if dist_sq < radius * radius {
                                            let falloff = 1.0 - (dist_sq.sqrt() / radius).min(1.0);
                                            let effect_damage = 6.0 * falloff as f32;
                                            if effect_damage > 0.1 {
                                                let _ = player_manager.apply_damage(&player.uuid, effect_damage, tick_count);
                                            }
                                            let _ = player_manager.add_effect(&player.uuid,
                                                mc_core::effect::ActiveEffect {
                                                    effect: mc_core::effect::EffectType::Slowness,
                                                    amplifier: 1,
                                                    duration_ticks: 200,
                                                });
                                        }
                                    }
                                    // Lingering: spawn AreaEffectCloud entity that persists
                                    if proj.projectile_type == mc_player::mob::ProjectileType::LingeringPotion {
                                        let cloud_eid = next_entity_id_for_tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        player_manager.broadcast_mob_spawn(cloud_eid, uuid::Uuid::new_v4(),
                                            mc_core::constants::entity_type::AREA_EFFECT_CLOUD, px, py, pz);
                                        // Track cloud for periodic effect application (stored in mob_manager)
                                        let cloud = mc_player::mob::TrackedMob {
                                            entity_id: cloud_eid, uuid: uuid::Uuid::new_v4(),
                                            mob_type: mc_core::constants::entity_type::AREA_EFFECT_CLOUD,
                                            position: mc_core::position::Position::new(px, py, pz),
                                            health: 1.0, max_health: 1.0, age_ticks: 0,
                                            ai_state: mc_player::mob::MobAiState::Idle, ai_timer: 600,
                                            attack_cooldown: 0, last_sync_tick: 0,
                                            owner_uuid: None, is_tamed: false, is_sitting: false,
                                            tame_attempts: 0, is_baby: false, in_love_ticks: 0,
                                            breed_cooldown: 0, is_sheared: false,
                                            path: vec![], path_last_tick: 0,
                                            is_on_fire: false, is_in_water: false,
                                        };
                                        mob_manager.register(cloud);
                                    }
                                }
                            // Loyalty trident: return to owner when expired
                            if proj.projectile_type == mc_player::mob::ProjectileType::Trident
                                && proj.owner_uuid != uuid::Uuid::nil() {
                                    // Return trident to owner inventory
                                    let trident_item = mc_core::block::BlockState::new(940); // trident item ID
                                    let _ = player_manager.add_item_to_player(&proj.owner_uuid, trident_item, 1);
                                }
                            // Channeling trident: spawn lightning on hit
                            if proj.projectile_type == mc_player::mob::ProjectileType::Trident
                                && proj.power_level > 0 // stored channeling flag
                                && proj.owner_uuid != uuid::Uuid::nil() {
                                    player_manager.broadcast_global(
                                        mc_player::player::PlayerStateEventKind::GameEventGlobal(3, 0.0));
                                    // Damage entities near hit location (3 block radius)
                                    for target in player_manager.all_players() {
                                        let dx = target.position.x - px;
                                        let dz = target.position.z - pz;
                                        if dx*dx + dz*dz < 9.0 {
                                            let _ = player_manager.apply_damage(&target.uuid, 5.0, tick_count);
                                        }
                                    }
                                }
                            // Handle arrow/fireball etc hit on players
                            if proj.owner_uuid != uuid::Uuid::nil() {
                                for player in player_manager.all_players() {
                                    let dx = player.position.x - px;
                                    let dy = (player.position.y + 1.0) - py;
                                    let dz = player.position.z - pz;
                                    if dx*dx + dy*dy + dz*dz < 2.25 {
                                        let _ = player_manager.apply_damage(&player.uuid, proj.damage, tick_count);
                                        if proj.flame_level > 0 {
                                            // Target set on fire for flame_level * 80 ticks
                                        }
                                    }
                                }
                            }
                        }
                }

                // Raid system tick (every 100 ticks = 5 seconds)
                if tick_count.is_multiple_of(100) {
                    for p in player_manager.all_players() {
                        let has_bad_omen = p.active_effects.iter().any(|e| e.effect.id() == 31);
                        if has_bad_omen {
                            // Clear BadOmen and trigger raid
                            player_manager.clear_effects(&p.uuid).ok();
                            if let Some(waves) = raid_manager.try_start_raid(p.uuid, &p.position, true, 2) {
                                info!("Raid started for player '{}' — {} waves incoming!", p.username, waves);
                                // Spawn initial wave immediately
                                let spawns = raid_manager.spawn_wave((p.position.x as i32, p.position.y as i32, p.position.z as i32));
                                for (mob_type, pos) in &spawns {
                                    let eid = next_entity_id_for_tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    let mob_uuid = uuid::Uuid::new_v4();
                                    let tracked = mc_player::mob::TrackedMob {
                                        entity_id: eid, uuid: mob_uuid,
                                        mob_type: *mob_type,
                                        position: mc_core::position::Position::new(pos.x, pos.y, pos.z),
                                        health: 20.0, max_health: 20.0, age_ticks: 0,
                                        ai_state: mc_player::mob::MobAiState::Idle, ai_timer: 40,
                                        attack_cooldown: 0, last_sync_tick: 0,
                                        owner_uuid: None, is_tamed: false, is_sitting: false,
                                        tame_attempts: 0, is_baby: false, in_love_ticks: 0,
                                        breed_cooldown: 0, is_sheared: false,
                                        path: vec![], path_last_tick: 0,
                                        is_on_fire: false, is_in_water: false,
                                    };
                                    mob_manager.register(tracked);
                                    player_manager.broadcast_mob_spawn(eid, mob_uuid, *mob_type, pos.x, pos.y, pos.z);
                                }
                            }
                        }
                    }
                }

                // Update mob pathfinding (every 40 ticks)
                if tick_count.is_multiple_of(40) {
                    tick::tick_mob_pathfinding(&player_manager, &mob_manager, &chunk_store, tick_count);
                }

                // BossBar auto-sync (every 20 ticks)
                if tick_count.is_multiple_of(20) {
                    tick::tick_bossbar_sync(&player_manager, &mob_manager);
                }

                // Nether portal detection (every 20 ticks)
                if tick_count.is_multiple_of(20) {
                    tick::tick_portal_detection(&player_manager, &chunk_store, &advancement_tracker, &advancement_registry);
                }

                // Item entity attraction: pull dropped items toward nearby players
                {
                    let mut drops = dropped_for_tick.write();
                    for player in player_manager.all_players() {
                        let px = player.position.x; let py = player.position.y; let pz = player.position.z;
                        for (item_id, ix, iy, iz) in drops.values_mut() {
                            let dx = px - *ix; let dy = (py + 1.0) - *iy; let dz = pz - *iz;
                            let dist_sq = dx*dx + dy*dy + dz*dz;
                            if dist_sq < 64.0 && dist_sq > 0.0 { // within 8 blocks
                                let dist = dist_sq.sqrt();
                                let speed = if dist_sq < 2.25 { 0.5 } else { 0.15 }; // fast close, slow far
                                *ix += dx / dist * speed;
                                *iy += dy / dist * speed;
                                *iz += dz / dist * speed;
                            }
                            // Remove item if player is within 0.5 blocks
                            if dist_sq < 0.25 {
                                let block = mc_core::block::BlockState::new(*item_id);
                                let _ = player_manager.add_item_to_player(&player.uuid, block, 1);
                                // mark for removal (handled by despawn in next pass)
                            }
                        }
                    }
                }
                // Patrol spawning (every 12000 ticks = 10 min)
                if tick_count.is_multiple_of(12000) {
                    tick::tick_patrol_spawning(&player_manager, &mob_manager, &next_entity_id_for_tick);
                }
                // AreaEffectCloud: auto-despawn after lifespan (clouds registered with ai_timer=600)
                // Cloud effects applied via the existing mob AI tick system

                // XP orb absorption (every tick)
                tick::tick_xp_absorption(&player_manager, &dropped_for_tick);

                // Mob combat: chasing mobs damage nearby players
                for mob in mob_manager.get_chasing() {
                    if mob.attack_cooldown > 0 { continue; }
                    if let MobAiState::Chasing { target_uuid } = mob.ai_state
                        && let Some(player) = player_manager.get(&target_uuid) {
                            let dx = player.position.x - mob.position.x;
                            let dz = player.position.z - mob.position.z;
                            let dist = (dx*dx + dz*dz).sqrt();
                            if dist < 2.0 {
                                let damage = match mob.mob_type {
                                    35 => 3.0,  // spider
                                    36 => 3.0,  // zombie
                                    37 => 2.0,  // skeleton (melee)
                                    49 => 4.0,  // wither_skeleton (stone sword)
                                    50 => 3.0,  // zombie_villager
                                    51 => 5.0,  // vindicator (iron axe)
                                    60 => 7.0,  // piglin_brute (golden axe)
                                    111 => 3.0, // husk
                                    _ => 2.0,
                                };
                                if player_manager.can_take_damage(&target_uuid, tick_count) {
                                    // Absorption: golden hearts absorb damage first
                                    let effective_damage = if player.absorption_health > 0.0f32 {
                                        let absorbed = if damage < player.absorption_health { damage } else { player.absorption_health };
                                        damage - absorbed
                                    } else { damage };
                                    let new_hp = (player.health - effective_damage).max(0.0);
                                    let _ = player_manager.set_health(&target_uuid, new_hp);
                                    player_manager.mark_damage_taken(&target_uuid, tick_count);
                                    // Hit effects
                                    if mob.mob_type == 49 {
                                        let _ = player_manager.add_effect(&target_uuid, mc_core::effect::ActiveEffect { effect: mc_core::effect::EffectType::Wither, amplifier: 0, duration_ticks: 140 });
                                    }
                                    if mob.mob_type == 111 {
                                        let _ = player_manager.add_effect(&target_uuid, mc_core::effect::ActiveEffect { effect: mc_core::effect::EffectType::Hunger, amplifier: 0, duration_ticks: 140 });
                                    }
                                }
                            }
                            // Ranged attacks
                            if (mob.mob_type == 37 || mob.mob_type == 112) && (4.0..15.0).contains(&dist)
                                && player_manager.can_take_damage(&target_uuid, tick_count) {
                                    let arrow_dmg = if mob.mob_type == 112 { 4.0 } else { 3.0 }; // stray slowness arrows hit harder
                                    let new_hp = (player.health - arrow_dmg).max(0.0);
                                    let _ = player_manager.set_health(&target_uuid, new_hp);
                                    player_manager.mark_damage_taken(&target_uuid, tick_count);
                                    if mob.mob_type == 112 {
                                        let _ = player_manager.add_effect(&target_uuid, mc_core::effect::ActiveEffect { effect: mc_core::effect::EffectType::Slowness, amplifier: 0, duration_ticks: 600 });
                                    }
                                }
                        }
                    // Creeper explosion
                    if let MobAiState::AboutToExplode { fuse_ticks } = mob.ai_state
                        && fuse_ticks == 0 {
                            // Apply damage to all nearby players
                            let mx = mob.position.x; let my = mob.position.y; let mz = mob.position.z;
                            for player in player_manager.all_players() {
                                let d = ((player.position.x - mx).powi(2) as f32 + (player.position.y - my).powi(2) as f32 + (player.position.z - mz).powi(2) as f32).sqrt();
                                if d < 4.0 {
                                    let dmg = (30.0f32 * (1.0 - d / 4.0)).max(0.0);
                                    if player_manager.can_take_damage(&player.uuid, tick_count) {
                                        let new_hp = (player.health - dmg).max(0.0);
                                        let _ = player_manager.set_health(&player.uuid, new_hp);
                                        player_manager.mark_damage_taken(&player.uuid, tick_count);
                                    }
                                }
                            }
                            // Destroy blocks in creeper explosion radius
                            let (mx, my, mz) = (mob.position.x as i32, mob.position.y as i32, mob.position.z as i32);
                            let mut creeper_affected_chunks: Vec<mc_core::position::ChunkPos> = Vec::new();
                            for dx in -3..=3i32 { for dy in -3..=3i32 { for dz in -3..=3i32 {
                                let d = ((dx*dx + dy*dy + dz*dz) as f32).sqrt();
                                if d <= 3.0 {
                                    let (wx, wy, wz) = (mx + dx, my + dy, mz + dz);
                                    if (-64..=319).contains(&wy) {
                                        let cp = mc_core::position::ChunkPos::new(wx >> 4, wz >> 4);
                                        if let Some(mut chunk) = chunk_store.get_mut(&cp) {
                                            let block = chunk.get_block((wx & 0xF) as usize, wy, (wz & 0xF) as usize);
                                            if !block.is_air() && block.id != 266 && d <= 3.0 * (1.0 - fastrand::f64() * 0.3) as f32 {
                                                chunk.set_block((wx & 0xF) as usize, wy, (wz & 0xF) as usize, mc_core::block::BlockState::AIR);
                                                dirty_blocks_for_tick.mark_block(wx, wy, wz, mc_core::block::BlockState::AIR.id);
                                                if !creeper_affected_chunks.contains(&cp) {
                                                    creeper_affected_chunks.push(cp);
                                                }
                                            }
                                        }
                                    }
                                }
                            }}}
                            for cp in &creeper_affected_chunks {
                                dirty_chunks_for_tick.write().insert(*cp);
                            }
                            mob_manager.remove(mob.entity_id);
                        }
                }

                // Death check: players with 0 health or below — drop inventory + XP
                {
                    let dead_uuids: Vec<uuid::Uuid> = player_manager.all_players()
                        .into_iter().filter(|p| p.health <= 0.0).map(|p| p.uuid).collect();
                    for uuid in &dead_uuids {
                        // Collect inventory and position before mutating
                        let (inv_items, inv_armor, pos, xp_total, spawn_pos) = {
                            if let Some(p) = player_manager.get(uuid) {
                                let items: Vec<_> = p.inventory.items.iter().filter_map(|s| s.clone()).collect();
                                let armor: Vec<_> = p.inventory.armor.iter().filter_map(|s| s.clone()).collect();
                                let sp = p.spawn_position.map(|(x,y,z,_)| (x,y,z))
                                    .unwrap_or_else(|| {
                                        let ws = world_state.read();
                                        (ws.spawn_x, ws.spawn_y, ws.spawn_z)
                                    });
                                (items, armor, (p.position.x, p.position.y, p.position.z), p.xp_total, sp)
                            } else { continue; }
                        };
                        let mut ent_id = next_entity_id_for_tick.load(std::sync::atomic::Ordering::Relaxed);
                        // Drop inventory items
                        for stack in &inv_items {
                            let eid = ent_id; ent_id += 1;
                            let ox = pos.0 + (fastrand::f64() - 0.5);
                            let oz = pos.2 + (fastrand::f64() - 0.5);
                            dropped_for_tick.write().insert(eid, (stack.item.id, ox, pos.1 + 0.5, oz));
                            player_manager.broadcast_mob_spawn(eid, uuid::Uuid::new_v4(), 54, ox, pos.1 + 0.5, oz);
                        }
                        // Drop armor
                        for stack in &inv_armor {
                            let eid = ent_id; ent_id += 1;
                            let ox = pos.0 + (fastrand::f64() - 0.5);
                            let oz = pos.2 + (fastrand::f64() - 0.5);
                            dropped_for_tick.write().insert(eid, (stack.item.id, ox, pos.1 + 0.5, oz));
                            player_manager.broadcast_mob_spawn(eid, uuid::Uuid::new_v4(), 54, ox, pos.1 + 0.5, oz);
                        }
                        // Drop XP orbs
                        let xp_drop = xp_total / 2;
                        let orb_count = (xp_drop / 7).clamp(1, 20);
                        for _ in 0..orb_count {
                            let eid = ent_id; ent_id += 1;
                            let ox = pos.0 + (fastrand::f64() - 0.5) * 2.0;
                            let oz = pos.2 + (fastrand::f64() - 0.5) * 2.0;
                            dropped_for_tick.write().insert(eid, (0, ox, pos.1 + 1.0, oz));
                            player_manager.broadcast_mob_spawn(eid, uuid::Uuid::new_v4(), 53, ox, pos.1 + 1.0, oz);
                        }
                        next_entity_id_for_tick.store(ent_id, std::sync::atomic::Ordering::Relaxed);
                        // Respawn
                        let _ = player_manager.set_health(uuid, 20.0);
                        let _ = player_manager.update_position_full(uuid, spawn_pos.0, spawn_pos.1, spawn_pos.2, 0.0, 0.0);
                        let _ = player_manager.set_food(uuid, 20, 5.0);
                        let _ = player_manager.set_inventory(uuid, mc_player::inventory::Inventory::new());
                    }
                }

                // Periodic hostile mob spawning (every 100 ticks)
                if tick_count.is_multiple_of(100) {
                    let ws = world_state.read();
                    let can_spawn = ws.game_rules.get("doMobSpawning").map(|v| v == "true").unwrap_or(false)
                        && ws.daytime >= 13000 && ws.daytime <= 23000;
                    let is_peaceful = matches!(ws.difficulty, mc_core::world_state::Difficulty::Peaceful);
                    drop(ws);
                    if can_spawn && !is_peaceful {
                        let hostile_count = mob_manager.count_hostile();
                        let max_hostile = 50 + player_manager.online_count() * 10;
                        if hostile_count < max_hostile {
                            for player in player_manager.all_players() {
                                if !fastrand::u32(..).is_multiple_of(3) { continue; }
                                let angle = fastrand::f64() * std::f64::consts::TAU;
                                let dist = 8.0 + fastrand::f64() * 16.0;
                                let sx = player.position.x + angle.cos() * dist;
                                let sz = player.position.z + angle.sin() * dist;
                                // Get surface height from chunk
                                let cp = mc_core::position::ChunkPos::new((sx as i32).div_euclid(16), (sz as i32).div_euclid(16));
                                let spawn_y = if let Some(chunk) = chunk_store.get(&cp) {
                                    let lx = (sx as i32).rem_euclid(16) as usize;
                                    let lz = (sz as i32).rem_euclid(16) as usize;
                                    let h = chunk.height_at(lx, lz);
                                    // Light check: hostile mobs require max(sky, block) <= 7
                                    let light = chunk.combined_light(lx, h - 1, lz);
                                    if light > 7 { continue; } // too bright for hostile spawn
                                    // Surface check
                                    if !chunk.is_spawn_surface(lx, h - 1, lz) { continue; }
                                    h as f64
                                } else {
                                    64.0 // fallback
                                };
                                let biome = mc_world::generator::sample_biome(sx as i32, sz as i32, world_state.read().seed);
                                let mob_type: i32 = {
                                    let r = fastrand::u32(..) % 16;
                                    match r {
                                        0 => 36, // zombie
                                        1 => 37, // skeleton
                                        2 => 33, // creeper
                                        3 => 35, // spider
                                        4 => 34, // slime
                                        5 => 38, // enderman
                                        6 => match biome { mc_core::biome::BiomeId::Desert | mc_core::biome::BiomeId::Badlands | mc_core::biome::BiomeId::ErodedBadlands | mc_core::biome::BiomeId::WoodedBadlands => 46, _ => 36 },
                                        7 => match biome { mc_core::biome::BiomeId::SnowyPlains | mc_core::biome::BiomeId::IceSpikes | mc_core::biome::BiomeId::SnowyTaiga | mc_core::biome::BiomeId::FrozenPeaks | mc_core::biome::BiomeId::SnowySlopes => 47, _ => 37 },
                                        8 => 45, // drowned
                                        9 => 48, // witch
                                        10 => 43, // blaze (nether-only, but spawns near lava)
                                        11 => 44, // fox (taiga)
                                        12 => 46, // cave_spider
                                        13 => 47, // silverfish
                                        14 => 55, // magma_cube (nether/lava)
                                        15 => if biome.is_nether() { 58 } else { 57 }, // hoglin or zombie_pigman
                                        16 => 71, // breeze (caves/underground)
                                        17 => 72, // bogged (swamp)
                                        _ => 36,
                                    }
                                };
                                let eid = next_entity_id_for_tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let mob_uuid = uuid::Uuid::new_v4();
                                let tracked = mc_player::mob::TrackedMob {
                                    entity_id: eid, uuid: mob_uuid, mob_type,
                                    position: mc_core::position::Position::new(sx, spawn_y, sz),
                                    health: mc_player::mob::mob_max_health(mob_type),
                                    max_health: mc_player::mob::mob_max_health(mob_type),
                                    age_ticks: 0, ai_timer: 0,
                                    ai_state: MobAiState::Idle,
                                    attack_cooldown: 0, last_sync_tick: 0,
                                    owner_uuid: None,
                                    is_tamed: false,
                                    is_sitting: false,
                                    tame_attempts: 0,
                                    is_baby: false,
                                    in_love_ticks: 0,
                                    breed_cooldown: 0,
                                    is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0,
                                };
                                mob_manager.register(tracked);
                                player_manager.broadcast_mob_spawn(eid, mob_uuid, mob_type, sx, spawn_y, sz);
                            }
                        }
                    }
                }

                // Passive mob spawning (every 200 ticks, daytime)
                if tick_count.is_multiple_of(200) {
                    let ws = world_state.read();
                    let can_spawn = ws.game_rules.get("doMobSpawning").map(|v| v == "true").unwrap_or(false)
                        && ws.daytime < 13000;
                    let is_peaceful = matches!(ws.difficulty, mc_core::world_state::Difficulty::Peaceful);
                    drop(ws);
                    if can_spawn && !is_peaceful {
                        let passive_count = mob_manager.count() - mob_manager.count_hostile();
                        let max_passive = 30 + player_manager.online_count() * 5;
                        if passive_count < max_passive {
                            for player in player_manager.all_players() {
                                if !fastrand::u32(..).is_multiple_of(5) { continue; }
                                let angle = fastrand::f64() * std::f64::consts::TAU;
                                let dist = 24.0 + fastrand::f64() * 24.0;
                                let sx = player.position.x + angle.cos() * dist;
                                let sz = player.position.z + angle.sin() * dist;
                                let cp = mc_core::position::ChunkPos::new((sx as i32).div_euclid(16), (sz as i32).div_euclid(16));
                                let spawn_y = if let Some(chunk) = chunk_store.get(&cp) {
                                    let lx = (sx as i32).rem_euclid(16) as usize;
                                    let lz = (sz as i32).rem_euclid(16) as usize;
                                    let h = chunk.height_at(lx, lz);
                                    // Light check: passive mobs need sky_light >= 9
                                    let section_idx = mc_world::chunk::section_index(h);
                                    if let Some(Some(sec)) = chunk.sections.get(section_idx) {
                                        let ly = h.rem_euclid(16) as usize;
                                        if sec.get_sky_light(lx, ly, lz) < 9 { continue; }
                                    }
                                    if !chunk.is_spawn_surface(lx, h - 1, lz) { continue; }
                                    h as f64
                                } else {
                                    64.0
                                };
                                let passive_types = [11i32, 12, 13, 14, 15, 16, 17, 18, 20, 21, 26, 27, 29, 30, 31, 32, 64, 66, 67, 70, 92, 98, 103];
                                let mob_type = passive_types[fastrand::usize(0..passive_types.len())];
                                let eid = next_entity_id_for_tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let mob_uuid = uuid::Uuid::new_v4();
                                let tracked = mc_player::mob::TrackedMob {
                                    entity_id: eid, uuid: mob_uuid, mob_type,
                                    position: mc_core::position::Position::new(sx, spawn_y, sz),
                                    health: mc_player::mob::mob_max_health(mob_type),
                                    max_health: mc_player::mob::mob_max_health(mob_type),
                                    age_ticks: 0, ai_timer: 40 + fastrand::u64(..) % 61,
                                    ai_state: MobAiState::Idle, attack_cooldown: 0, last_sync_tick: 0,
                                    owner_uuid: None, is_tamed: false, is_sitting: false, tame_attempts: 0, is_baby: false, in_love_ticks: 0, breed_cooldown: 0, is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0,
                                };
                                mob_manager.register(tracked);
                                player_manager.broadcast_mob_spawn(eid, mob_uuid, mob_type, sx, spawn_y, sz);
                            }
                        }
                    }
                }

                // Villager restock check (every 2400 ticks = 2 min)
                if tick_count.is_multiple_of(2400) {
                    for eid in mob_manager.all_entity_ids() {
                        if let Some(mob) = mob_manager.get(eid)
                            && mob.mob_type == 92 { // villager
                                if let Some(mut data) = villager_data.get_mut(&eid)
                                    && data.tick_restock(tick_count) {
                                        tracing::debug!("Villager {} restocked trades (level {})", eid, data.level);
                                    }
                            }
                    }
                }

                // Villager breeding check (every 600 ticks)
                if tick_count.is_multiple_of(600) {
                    let villagers: Vec<mc_player::mob::TrackedMob> = mob_manager
                        .all_entity_ids().iter()
                        .filter_map(|eid| mob_manager.get(*eid))
                        .filter(|m| m.mob_type == 92)
                        .collect();
                    if villagers.len() >= 2 {
                        for i in 0..villagers.len().saturating_sub(1) {
                            let v1 = &villagers[i];
                            if let Some(v2) = villagers.get(i + 1) {
                                let dx = v1.position.x - v2.position.x;
                                let dz = v1.position.z - v2.position.z;
                                if dx*dx + dz*dz < 16.0 { // within 4 blocks
                                    let baby_eid = next_entity_id_for_tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    let baby = mc_player::mob::TrackedMob {
                                        entity_id: baby_eid, uuid: uuid::Uuid::new_v4(), mob_type: 92,
                                        position: mc_core::position::Position::new(
                                            v1.position.x, v1.position.y, v1.position.z),
                                        health: 20.0, max_health: 20.0,
                                        age_ticks: 0, ai_timer: 200,
                                        ai_state: MobAiState::Idle, attack_cooldown: 0, last_sync_tick: 0,
                                        owner_uuid: None, is_tamed: false, is_sitting: false, tame_attempts: 0, is_baby: false, in_love_ticks: 0, breed_cooldown: 0, is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0,
                                    };
                                    mob_manager.register(baby);
                                    break; // one baby per cycle
                                }
                            }
                        }
                    }
                    // Iron golem spawning chance
                    if villagers.len() >= 10 && fastrand::u32(..).is_multiple_of(7000)
                        && let Some(v) = villagers.first() {
                            let golem_eid = next_entity_id_for_tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let golem = mc_player::mob::TrackedMob {
                                entity_id: golem_eid, uuid: uuid::Uuid::new_v4(), mob_type: 99,
                                position: mc_core::position::Position::new(v.position.x, 64.0, v.position.z),
                                health: 100.0, max_health: 100.0,
                                age_ticks: 0, ai_timer: 80,
                                ai_state: MobAiState::Idle, attack_cooldown: 0, last_sync_tick: 0,
                                owner_uuid: None, is_tamed: false, is_sitting: false, tame_attempts: 0, is_baby: false, in_love_ticks: 0, breed_cooldown: 0, is_sheared: false, is_on_fire: false, is_in_water: false, path: Vec::new(), path_last_tick: 0,
                            };
                            mob_manager.register(golem);
                        }
                }

                // World border damage check (every 20 ticks)
                if tick_count.is_multiple_of(20) {
                    let ws = world_state.read();
                    let border = ws.world_border.clone();
                    drop(ws);
                    let half = border.size / 2.0;
                    for player in player_manager.all_players() {
                        let dx = (player.position.x - border.center_x).abs();
                        let dz = (player.position.z - border.center_z).abs();
                        let outside = dx > half || dz > half;
                        if outside {
                            let dist_outside = (dx.max(dz)) - half;
                            // Push player back toward center
                            let push_x = if player.position.x > border.center_x { -0.5 } else if player.position.x < border.center_x { 0.5 } else { 0.0 };
                            let push_z = if player.position.z > border.center_z { -0.5 } else if player.position.z < border.center_z { 0.5 } else { 0.0 };
                            let _ = player_manager.update_position_full(
                                &player.uuid,
                                player.position.x + push_x,
                                player.position.y,
                                player.position.z + push_z,
                                player.position.yaw,
                                player.position.pitch,
                            );
                            if dist_outside > border.safe_zone {
                                let damage = border.damage_per_block;
                                let new_hp = (player.health - damage as f32).max(0.0);
                                let _ = player_manager.set_health(&player.uuid, new_hp);
                            }
                        }
                    }
                }

                // Periodic dirty block tracker cleanup (overflow protection)
                dirty_blocks_for_tick.cleanup_stale(tick_count);

                // Periodic rate limiter stale entry cleanup (every 300 ticks = 15s)
                if tick_count.is_multiple_of(300) {
                    mc_network::rate_limiter::cleanup_stale_rate_limits();
                }

                // 定期自动保存 (LZ4 Linear 格式优先) — 异步 I/O
                if save_interval_ticks > 0 && tick_count.is_multiple_of(save_interval_ticks) {
                    let dirty = chunk_store.dirty_chunks();
                    if !dirty.is_empty() {
                        let wd = world_dir.clone();
                        let cs = chunk_store.clone();
                        let tc = tick_count;
                        // Offload disk I/O to blocking thread pool
                        io_handle.spawn_blocking(move || {
                            let count = mc_world::chunk_store::save_dirty_chunks_linear(&dirty, &wd);
                            if count > 0 {
                                tracing::debug!("LZ4 async-saved {} chunks (tick {})", count, tc);
                                cs.mark_all_clean();
                            } else {
                                let chunks: Vec<mc_world::chunk::Chunk> = dirty.into_iter().map(|(_, c)| c).collect();
                                let writer = mc_world::anvil::AnvilWriter::new();
                                match writer.write_chunks(&wd, &chunks) {
                                    Ok(n) => { tracing::debug!("Anvil async-saved {} chunks (tick {})", n, tc); cs.mark_all_clean(); }
                                    Err(e) => tracing::error!("Auto-save failed: {}", e),
                                }
                            }
                        });
                    }
                    // 保存玩家数据 (完整状态)
                    for player in player_manager.all_players() {
                        save_manager.save_player_full(
                            &player.uuid, &player.username,
                            player.health, player.food_level, player.food_saturation,
                            player.gamemode.id(),
                            player.position.x, player.position.y, player.position.z,
                            player.position.yaw, player.position.pitch,
                            player.is_op,
                            Some(player.inventory.serialize()),
                        );
                        // Also persist OP status to ops table
                        save_manager.persist_op(&player.uuid, player.is_op);
                    }
                    // Save container contents to disk (atomic write to prevent corruption)
                    let container_data = container_manager.serialize_all();
                    let containers_path = server_root.join("data").join("containers.bin");
                    let containers_tmp = server_root.join("data").join("containers.bin.tmp");
                    if let Err(e) = std::fs::write(&containers_tmp, &container_data) {
                        tracing::warn!("Failed to write containers.tmp: {}", e);
                    } else if let Err(e) = std::fs::rename(&containers_tmp, &containers_path) {
                        tracing::warn!("Failed to rename containers.tmp to containers.bin: {}", e);
                    }
                    // Persist bans and whitelist
                    let banned = player_manager.get_banned();
                    save_manager.persist_bans(&banned);
                    let wl = player_manager.get_whitelist_entries();
                    save_manager.persist_whitelist(&wl);
                    let world = world_arc.read();
                    save_manager.save_world(&world);
                }
                // Record tick duration for TPS metrics
                let tick_elapsed_us = tick_start.elapsed().as_micros() as u64;
                metrics::record_tick(tick_elapsed_us);
            }
            _ = save_trigger_rx.recv() => {
                // Manual save triggered by /save-all command
                info!("Manual save triggered");
                let dirty: Vec<(mc_core::position::ChunkPos, mc_world::chunk::Chunk)> = chunk_store.dirty_chunks();
                if !dirty.is_empty() {
                    mc_world::chunk_store::save_dirty_chunks_linear(&dirty, &world_dir);
                }
                for player in player_manager.all_players() {
                    save_manager.save_player_full(&player.uuid, &player.username, player.health, player.food_level, player.food_saturation, player.gamemode.id(), player.position.x, player.position.y, player.position.z, player.position.yaw, player.position.pitch, player.is_op, Some(player.inventory.serialize()));
                }
                save_manager.persist_bans(&player_manager.get_banned());
                save_manager.persist_whitelist(&player_manager.get_whitelist_entries());
                info!("Manual save completed");
            }
            _ = shutdown_rx_for_tick.recv() => {
                info!("Shutdown signal received, saving...");
                break;
            }
        }
    }

    // 保存修改后的区块到 LZ4 Linear (not Anvil — avoids block ID corruption)
    {
        let dirty: Vec<(mc_core::position::ChunkPos, mc_world::chunk::Chunk)> = chunk_store.dirty_chunks();
        if !dirty.is_empty() {
            let count = mc_world::chunk_store::save_dirty_chunks_linear(&dirty, &world_dir);
            info!("Saved {} modified chunks to {}", count, world_dir.display());
        }
        // 保存玩家数据 (完整状态)
        for player in player_manager.all_players() {
            save_manager.save_player_full(
                &player.uuid, &player.username,
                player.health, player.food_level, player.food_saturation,
                player.gamemode.id(),
                player.position.x, player.position.y, player.position.z,
                player.position.yaw, player.position.pitch,
                player.is_op,
                Some(player.inventory.serialize()),
            );
            save_manager.persist_op(&player.uuid, player.is_op);
        }
        // Persist bans/whitelist
        save_manager.persist_bans(&player_manager.get_banned());
        save_manager.persist_whitelist(&player_manager.get_whitelist_entries());
        let world = world_arc.read();
        save_manager.save_world(&world);
        info!("World saved.");
    }
    info!("Server stopped. Goodbye!");
    Ok(())
}

/// 接受连接循环 (带并发限制)
async fn accept_loop(
    listener: ServerListener,
    server_ref: connection::ServerRef,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    semaphore: Arc<tokio::sync::Semaphore>,
) {
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let srv = server_ref.clone();
                        let permit = semaphore.clone();
                        info!("Incoming connection from {}", addr);
                        let handle = tokio::runtime::Handle::current();
                        std::thread::spawn(move || {
                            handle.block_on(async move {
                                let _permit = permit.acquire().await;
                                connection::handle_connection(stream, srv).await;
                            });
                        });
                    }
                    Err(e) => error!("Accept error: {}", e),
                }
            }
            _ = shutdown_rx.recv() => {
                info!("Accept loop shutting down");
                return;
            }
        }
    }
}



