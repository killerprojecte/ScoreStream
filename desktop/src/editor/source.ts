import * as monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import type { Diagnostic } from '../types';
(self as unknown as {MonacoEnvironment:unknown}).MonacoEnvironment={getWorker:()=>new EditorWorker()};
monaco.languages.register({id:'scorestream'});
monaco.languages.setMonarchTokensProvider('scorestream',{tokenizer:{root:[[/^\s*;.*$/,'comment'],[/![\w-]+/,'keyword'],[/@[\d/]+/,'number'],[/#[\w.-]+/,'type'],[/\b[A-G](?:[#b]+|\([^)]*\))?-?\d+/,'string'],[/\b(?:id|part|staff|instrument|staves|len|chord|unit|meter|clef|key|tempo|w|dots)=/,'attribute.name'],[/"(?:[^"\\]|\\.)*"/,'string'],[/\b\d+(?:\/\d+)?\b/,'number'],[/^[.=|+~]/,'keyword']]}});
monaco.editor.defineTheme('scorestream',{base:'vs',inherit:true,rules:[{token:'keyword',foreground:'7956A5'},{token:'string',foreground:'287A66'},{token:'number',foreground:'4D6C9E'},{token:'comment',foreground:'9197A1'},{token:'attribute.name',foreground:'707885'}],colors:{'editor.background':'#FBFBFC','editor.foreground':'#3C424E','editorLineNumber.foreground':'#A4A9B3','editorLineNumber.activeForeground':'#596E9A','editor.lineHighlightBackground':'#F0F2F6','editor.selectionBackground':'#DAE5F8','editorCursor.foreground':'#596E9A','editorGutter.background':'#FBFBFC','editorIndentGuide.background1':'#ECEEF3','editorWidget.background':'#FFFFFF'}});
export class SourceEditor {
  readonly editor:monaco.editor.IStandaloneCodeEditor;
  private silent=false;
  constructor(host:HTMLElement,onChange:()=>void,onSelection:(line:number)=>void){
    this.editor=monaco.editor.create(host,{value:'',language:'scorestream',theme:'scorestream',fontFamily:'"DejaVu Sans Mono", "SFMono-Regular", Consolas, monospace',fontSize:12,lineHeight:22,minimap:{enabled:false},scrollBeyondLastLine:false,wordWrap:'off',automaticLayout:true,renderLineHighlight:'line',overviewRulerLanes:0,hideCursorInOverviewRuler:true,glyphMargin:false,folding:false,lineNumbersMinChars:3,padding:{top:14,bottom:20},tabSize:2,stickyScroll:{enabled:false},accessibilitySupport:'on'});
    this.editor.onDidChangeModelContent(()=>{if(!this.silent)onChange();});
    this.editor.onDidChangeCursorPosition(e=>{if(!this.silent)onSelection(e.position.lineNumber);});
  }
  get value(){return this.editor.getValue();}
  set value(value:string){this.silent=true;this.editor.setValue(value);this.silent=false;}
  set readOnly(value:boolean){this.editor.updateOptions({readOnly:value});}
  diagnostics(items:Diagnostic[]){monaco.editor.setModelMarkers(this.editor.getModel()!,'rust',items.map(d=>({startLineNumber:Math.max(d.line,1),startColumn:Math.max(d.column,1),endLineNumber:Math.max(d.line,1),endColumn:Math.max(d.column+1,2),message:`${d.code}: ${d.message}`,severity:d.severity==='warning'?monaco.MarkerSeverity.Warning:monaco.MarkerSeverity.Error})));}
  selectLine(line:number){this.silent=true;this.editor.setSelection({startLineNumber:line,startColumn:1,endLineNumber:line,endColumn:1e3});this.editor.revealLineInCenterIfOutsideViewport(line);this.silent=false;}
  focus(){this.editor.focus();}
}
