import './style.css';
import {createRoot} from 'react-dom/client';
import {App} from './App';
import './glass/glass.css';
import './glass/application.css';
import './reference-v0.css';
import {Workbench} from './app/workbench';
import {PreferenceStore} from './preferences-model';
let storage:Storage|null=null;try{storage=window.localStorage;}catch{}
const preferences=new PreferenceStore(storage,navigator.language);
const workbench=new Workbench();
createRoot(document.getElementById('app')!).render(<App controller={workbench} store={preferences}/>);
if((window as unknown as {__SCORESTREAM_SMOKE__?:boolean}).__SCORESTREAM_SMOKE__)void import('./smoke').then(m=>m.run(workbench,preferences));

if((window as unknown as {__SCORESTREAM_V0_CONTROL_QA__?:boolean}).__SCORESTREAM_V0_CONTROL_QA__)void import('./testing/control-geometry-v0').then(m=>m.observeControls());
