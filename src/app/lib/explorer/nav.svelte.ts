import { catalog, type Category, type Experiment } from "./catalog.js";

/** 当前选中的实验及其所属分类。 */
export interface Selection {
	category: Category;
	experiment: Experiment;
}

let selectedId = $state<string | null>(null);

function find(id: string | null): Selection | null {
	if (!id) return null;
	for (const category of catalog) {
		const experiment = category.experiments.find((e) => e.id === id);
		if (experiment) return { category, experiment };
	}
	return null;
}

/** 全局导航状态：null 表示首页。 */
export const nav = {
	get selectedId() {
		return selectedId;
	},
	get selected(): Selection | null {
		return find(selectedId);
	},
	select(id: string | null) {
		selectedId = id;
	},
};
