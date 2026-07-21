//! ServerContext — 命令系统可访问的统一服务器状态

use mc_command::dispatcher::CommandDispatcher;
use mc_core::world_state::SharedWorldState;
use mc_player::player::SharedPlayerManager;
use std::sync::Arc;
use tokio::sync::broadcast;

/// 服务器关闭信号（广播通道）
pub type ShutdownSignal = broadcast::Sender<()>;

/// 服务器上下文 — 命令执行时可访问的所有状态
pub struct ServerContext {
    pub player_manager: SharedPlayerManager,
    pub command_dispatcher: Arc<parking_lot::Mutex<CommandDispatcher>>,
    pub shutdown_tx: ShutdownSignal,
    pub world_state: SharedWorldState,
    pub motd: String,
    pub max_players: u32,
}

impl ServerContext {
    pub fn new(
        player_manager: SharedPlayerManager,
        shutdown_tx: ShutdownSignal,
        world_state: SharedWorldState,
        motd: String,
        max_players: u32,
    ) -> Self {
        let mut dispatcher = CommandDispatcher::new();
        register_commands(&mut dispatcher);

        Self {
            player_manager,
            command_dispatcher: Arc::new(parking_lot::Mutex::new(dispatcher)),
            shutdown_tx,
            world_state,
            motd,
            max_players,
        }
    }
}

/// 注册所有内置命令
fn register_commands(dispatcher: &mut CommandDispatcher) {
    use mc_command::commands::admin::*;
    use mc_command::commands::advanced::*;
    use mc_command::commands::execute::ExecuteCommand;
    use mc_command::commands::help::HelpCommand;
    use mc_command::commands::info::*;
    use mc_command::commands::msg::{MeCommand, MsgCommand};
    use mc_command::commands::player::*;
    use mc_command::commands::scoreboard::ScoreboardCommand;
    use mc_command::commands::world::*;

    dispatcher.register(ExecuteCommand);
    dispatcher.register(HelpCommand);
    dispatcher.register(MsgCommand);
    dispatcher.register(MeCommand);
    dispatcher.register(OpCommand);
    dispatcher.register(DeopCommand);
    dispatcher.register(StopCommand);
    dispatcher.register(KickCommand);
    dispatcher.register(BanCommand);
    dispatcher.register(PardonCommand);
    dispatcher.register(BanIpCommand);
    dispatcher.register(PardonIpCommand);
    dispatcher.register(SetIdleTimeoutCommand);
    dispatcher.register(ListPlayersCommand);
    dispatcher.register(BanlistCommand);
    dispatcher.register(WhitelistCommand);
    dispatcher.register(SaveAllCommand);
    dispatcher.register(ReloadCommand);
    dispatcher.register(TransferCommand);
    dispatcher.register(GamemodeCommand);
    dispatcher.register(DefaultGamemodeCommand);
    dispatcher.register(SetblockCommand);
    dispatcher.register(SpawnpointCommand);
    dispatcher.register(FillCommand);
    dispatcher.register(XpCommand);
    dispatcher.register(SummonCommand);
    dispatcher.register(EffectCommand);
    dispatcher.register(GameruleCommand);
    dispatcher.register(TpCommand);
    dispatcher.register(GiveCommand);
    dispatcher.register(KillCommand);
    dispatcher.register(TimeCommand);
    dispatcher.register(WeatherCommand);
    dispatcher.register(DifficultyCommand);
    dispatcher.register(TickCommand);
    dispatcher.register(ListPlayersCommand);
    dispatcher.register(SeedCommand);
    dispatcher.register(SayCommand);
    dispatcher.register(StatusCommand);
    dispatcher.register(ScoreboardCommand);
    // Advanced commands
    dispatcher.register(TitleCommand);
    dispatcher.register(PlaysoundCommand);
    dispatcher.register(ClearCommand);
    dispatcher.register(EnchantCommand);
    dispatcher.register(BossbarCommand);
    dispatcher.register(LocateCommand);
    dispatcher.register(CloneCommand);
    dispatcher.register(DamageCommand);
    dispatcher.register(ItemCommand);
    dispatcher.register(SetworldspawnCommand);
    dispatcher.register(SpreadplayersCommand);
    dispatcher.register(AttributeCommand);
    dispatcher.register(StopsoundCommand);
    dispatcher.register(RecipeCommand);
    dispatcher.register(SpectateCommand);
    dispatcher.register(WorldborderCommand);
    dispatcher.register(DataCommand);
    dispatcher.register(TagCommand);
    dispatcher.register(TeamCommand);
    dispatcher.register(TriggerCommand);
    dispatcher.register(RideCommand);
    dispatcher.register(FillbiomeCommand);
    dispatcher.register(ForceloadCommand);
    dispatcher.register(SaveOnCommand);
    dispatcher.register(SaveOffCommand);
    dispatcher.register(PublishCommand);
    dispatcher.register(DebugCommand);

    tracing::info!(
        "Registered {} commands",
        dispatcher.list_commands().len()
    );
}

impl Clone for ServerContext {
    fn clone(&self) -> Self {
        Self {
            player_manager: self.player_manager.clone(),
            command_dispatcher: self.command_dispatcher.clone(),
            shutdown_tx: self.shutdown_tx.clone(),
            world_state: self.world_state.clone(),
            motd: self.motd.clone(),
            max_players: self.max_players,
        }
    }
}
