import { mount } from 'svelte';
import '@fontsource-variable/inter-tight';
import '@fontsource-variable/geist-mono';
import './styles/global.css';
import App from './App.svelte';

const app = mount(App, { target: document.getElementById('app')! });

export default app;
