import * as monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import type { Diagnostic } from '../types';
(self as unknown as {MonacoEnvironment:unknown}).MonacoEnvironment={getWorker:()=>new EditorWorker()};
monaco.languages.register({id:'scorestream'});
monaco.languages.setMonarchTokensProvider('scorestream',{tokenizer:{root:[[/^\s*;.*$/,'comment'],[/![\w-]+/,'keyword'],[/@[\d/]+/,'number'],[/#[\w.-]+/,'type'],[/\b[A-G](?:[#b]+|\([^)]*\))?-?\d+/,'string'],[/\b(?:id|part|staff|instrument|staves|len|chord|unit|meter|clef|key|tempo|w|dots)=/,'attribute.name'],[/"(?:[^"\\]|\\.)*"/,'string'],[/\b\d+(?:\/\d+)?\b/,'number'],[/^[.=|+~]/,'keyword']]}});
monaco.editor.defineTheme('scorestream',{base:'vs',inherit:true,rules:[{token:'keyword',foreground:'B15038',fontStyle:'bold'},{token:'string',foreground:'396E60'},{token:'number',foreground:'7460A1'},{token:'comment',foreground:'99958B'},{token:'attribute.name',foreground:'817465'}],colors:{'editor.background':'#FAF9F6','editor.foreground':'#373A35','editorLineNumber.foreground':'#B1ADA4','editorLineNumber.activeForeground':'#B15038','editor.lineHighlightBackground':'#EFEEE7','editor.selectionBackground':'#DED8CD','editorCursor.foreground':'#B15038','editorGutter.background':'#FAF9F6','editorIndentGuide.background1':'#EEECE5','editorWidget.background':'#FFFFFF'}});
export class SourceEditor {
  readonly editor:monaco.editor.IStandaloneCodeEditor;
  private silent=false;
  constructor(host:HTMLElement,onChange:()=>void,onSelection:(line:number)=>void){
    this.editor=monaco.editor.create(host,{value:'',language:'scorestream',theme:'scorestream',fontFamily:'"DejaVu Sans Mono", "SFMono-Regular", Consolas, monospace',fontSize:12,lineHeight:21,minimap:{enabled:false},scrollBeyondLastLine:false,wordWrap:'off',automaticLayout:true,renderLineHighlight:'line',overviewRulerLanes:0,hideCursorInOverviewRuler:true,glyphMargin:false,folding:false,lineNumbersMinChars:3,padding:{top:20,bottom:24},tabSize:2,stickyScroll:{enabled:false},accessibilitySupport:'on'});
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
