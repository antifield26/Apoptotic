//! 队伍系统 — 管理玩家队伍 (颜色、前缀、友军伤害)

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// 队伍定义
#[derive(Debug, Clone)]
pub struct Team {
    pub name: String,
    pub color: String,       // "red", "blue", "green", "gold", etc.
    pub prefix: String,
    pub suffix: String,
    pub friendly_fire: bool,
    pub members: HashSet<Uuid>,
}

impl Team {
    pub fn new(name: &str, color: &str) -> Self {
        Self {
            name: name.to_string(), color: color.to_string(),
            prefix: String::new(), suffix: String::new(),
            friendly_fire: true, members: HashSet::new(),
        }
    }
}

/// 队伍管理器
#[derive(Debug, Clone)]
pub struct TeamManager {
    pub teams: HashMap<String, Team>,
    pub player_team: HashMap<Uuid, String>, // uuid → team_name
}

impl Default for TeamManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TeamManager {
    pub fn new() -> Self {
        Self { teams: HashMap::new(), player_team: HashMap::new() }
    }

    pub fn add_team(&mut self, name: &str, color: &str) {
        self.teams.entry(name.to_string()).or_insert_with(|| Team::new(name, color));
    }

    pub fn remove_team(&mut self, name: &str) {
        if let Some(team) = self.teams.remove(name) {
            for uuid in &team.members {
                self.player_team.remove(uuid);
            }
        }
    }

    pub fn join_team(&mut self, name: &str, uuid: &Uuid) -> bool {
        // Remove from old team first
        let old_name = self.player_team.get(uuid).cloned();
        if let Some(ref old) = old_name
            && let Some(old_team) = self.teams.get_mut(old) {
                old_team.members.remove(uuid);
            }
        // Add to new team
        if let Some(team) = self.teams.get_mut(name) {
            team.members.insert(*uuid);
            self.player_team.insert(*uuid, name.to_string());
            return true;
        }
        false
    }

    pub fn leave_team(&mut self, uuid: &Uuid) {
        if let Some(name) = self.player_team.remove(uuid)
            && let Some(team) = self.teams.get_mut(&name) {
                team.members.remove(uuid);
            }
    }

    pub fn get_team(&self, uuid: &Uuid) -> Option<&Team> {
        self.player_team.get(uuid).and_then(|name| self.teams.get(name))
    }

    pub fn list_teams(&self) -> Vec<&Team> {
        self.teams.values().collect()
    }
}
