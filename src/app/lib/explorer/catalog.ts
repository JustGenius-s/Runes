import type { Component } from "svelte";
import ComponentIcon from "@lucide/svelte/icons/component";
import PuzzleIcon from "@lucide/svelte/icons/puzzle";
import SigmaIcon from "@lucide/svelte/icons/sigma";

/** 一个可追踪的实验条目。 */
export interface Experiment {
	/** 全局唯一 id，用于导航选中态。 */
	id: string;
	title: string;
	/** 这个实验要量化什么。 */
	summary: string;
}

/** 侧边栏中的一个分类。 */
export interface Category {
	id: string;
	title: string;
	icon: Component;
	experiments: Experiment[];
}

export const catalog: Category[] = [
	{
		id: "algorithms",
		title: "算法",
		icon: SigmaIcon,
		experiments: [
			{
				id: "sorting",
				title: "排序对比",
				summary: "冒泡 / 快排 / 归并的比较次数、写入次数与派生链长度。",
			},
			{
				id: "binary-search",
				title: "二分查找",
				summary: "每一步的区间收敛路径与访问序列。",
			},
			{
				id: "graph-traversal",
				title: "图遍历",
				summary: "BFS / DFS 的访问顺序与队列、栈的实时变化。",
			},
			{
				id: "dp-fib",
				title: "动态规划",
				summary: "斐波那契：朴素递归 vs 记忆化的重复计算量对比。",
			},
		],
	},
	{
		id: "patterns",
		title: "设计模式",
		icon: PuzzleIcon,
		experiments: [
			{
				id: "pub-sub",
				title: "发布订阅",
				summary: "一次事件发布引发的订阅者通知链与扇出宽度。",
			},
			{
				id: "strategy",
				title: "策略模式",
				summary: "运行时切换策略后的调用分布与上下文依赖。",
			},
			{
				id: "state-machine",
				title: "状态机",
				summary: "状态迁移路径、迁移次数与非法迁移拦截记录。",
			},
			{
				id: "memento",
				title: "备忘录",
				summary: "撤销 / 重做的快照数量、体积与恢复路径。",
			},
		],
	},
	{
		id: "components",
		title: "组件",
		icon: ComponentIcon,
		experiments: [
			{
				id: "counter",
				title: "计数器",
				summary: "最小状态单元：一次点击触发的派生更新链。",
			},
			{
				id: "todo-list",
				title: "Todo 列表",
				summary: "增删改与过滤操作引发的派生计算范围。",
			},
			{
				id: "form-validation",
				title: "表单校验",
				summary: "字段间依赖关系与单次输入的校验触发面。",
			},
			{
				id: "store-compare",
				title: "状态管理对比",
				summary: "同一场景下不同状态管理方案的更新扇出对比。",
			},
		],
	},
];
