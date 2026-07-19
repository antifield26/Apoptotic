//! 铁砧系统 — 物品修复、合并、重命名
//!
//! 支持: 同类合并 + 材料修复 + 重命名（简化版，使用 BlockState）

use crate::inventory::ItemStack;

/// 铁砧计算结果
#[derive(Debug, Clone)]
pub struct AnvilResult {
    pub output: ItemStack,
    pub xp_cost: u32,
}

/// 铁砧管理器
pub struct AnvilManager;

impl AnvilManager {
    /// 计算铁砧操作结果
    pub fn calculate(left: &ItemStack, right: &ItemStack, new_name: Option<&str>) -> Option<AnvilResult> {
        if left.item.id == 0 || left.count == 0 { return None; }

        // 重命名
        if let Some(name) = new_name
            && !name.is_empty() {
                return Some(AnvilResult { output: left.clone(), xp_cost: 1 });
            }

        if right.item.id == 0 || right.count == 0 { return None; }

        // 同类物品合并 (工具/盔甲)
        if left.item.id == right.item.id && Self::is_tool(left.item.id) {
            return Some(AnvilResult { output: left.clone(), xp_cost: 2 });
        }

        // 材料修复: 铁锭修复铁工具, 钻石修复钻石工具
        if Self::is_repair_material(left.item.id, right.item.id) {
            return Some(AnvilResult { output: left.clone(), xp_cost: 1 });
        }

        None
    }

    fn is_tool(item_id: u32) -> bool {
        matches!(item_id,
            773..=786 | 940 | 941 | 787..=790
        )
    }

    fn is_repair_material(tool_id: u32, mat_id: u32) -> bool {
        match tool_id {
            775 | 783 | 787 | 788 | 789 | 790 => mat_id == 804, // iron tools → iron ingot
            777 | 785 | 786 => mat_id == 996, // diamond tools → diamond
            _ => false,
        }
    }
}
