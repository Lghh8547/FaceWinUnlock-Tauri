import { createRouter, createWebHashHistory } from 'vue-router'
import Init from '../views/Init.vue'
import MainLayout from '../layout/MainLayout.vue'
import Dashboard from '../views/Dashboard.vue'
import List from '../views/Faces/List.vue'
import Add from '../views/Faces/Add.vue'

const routes = [
	{ path: '/init', name: 'Init', component: Init, meta: { title: '系统初始化' }},
	{ 
		path: '/',
		component: MainLayout,
		children: [
			{
				path: '',
				name: 'Dashboard',
				component: Dashboard
			},{
				path: 'face',
				name: 'FaceList',
				component: List
			},{
				path: 'faces/add',
				name: 'FaceAdd',
				component: Add
			}
		]
	}
]

const router = createRouter({
	history: createWebHashHistory(),
	routes
});

export default router