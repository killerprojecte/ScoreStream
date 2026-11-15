import * as monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import type { Diagnostic } from '../types';
(self as unknown as {MonacoEnvironment:unknown}).MonacoEnvironment={getWorker:()=>new EditorWorker()};
monaco.languages.register({id:'scorestream'});
monaco.languages.setMonarchTokensProvider('scorestream',{tokenizer:{root:[[/^\s*;.*$/,'comment'],[/![\w-]+/,'keyword'],[/@[\d/]+/,'number'],[/#[\w.-]+/,'type'],[/\b[A-G](?:[#b]+|\([^)]*\))?-?\d+/,'string'],[/\b(?:id|part|staff|instrument|staves|display_name|len|chord|unit|meter|clef|key|tempo|w|dots|note_size)=/,'attribute.name'],[/"(?:[^"\\]|\\.)*"/,'string'],[/\b\d+(?:\/\d+)?\b/,'number'],[/^[.=|+~]/,'keyword']]}});
// Opaque reading surfaces keep syntax stable across blur, layered and solid modes.
export const editorPalettes = {
 light: {background:'#FAFAFC',foreground:'#36363A',comment:'#596579',keyword:'#70408D',string:'#246653',number:'#3E5F91',attribute:'#505F75',line:'#617086'},
 dark: {background:'#252527',foreground:'#ECECEF',comment:'#ABB8CC',keyword:'#D5B5F8',string:'#9FDFC4',number:'#AEC9FF',attribute:'#C5CEDD',line:'#A7B6CB'},
};
for (const [mode,p] of Object.entries(editorPalettes)) monaco.editor.defineTheme(`scorestream-${mode}`,{
 base:mode==='dark'?'vs-dark':'vs',inherit:true,
 rules:[{token:'keyword',foreground:p.keyword.slice(1)},{token:'string',foreground:p.string.slice(1)},{token:'number',foreground:p.number.slice(1)},{token:'comment',foreground:p.comment.slice(1)},{token:'attribute.name',foreground:p.attribute.slice(1)}],
 colors:{'editor.background':p.background,'editor.foreground':p.foreground,'editorLineNumber.foreground':p.line,'editorLineNumber.activeForeground':p.foreground,'editor.lineHighlightBackground':mode==='dark'?'#303034':'#EFEFF3','editor.selectionBackground':mode==='dark'?'#354F78':'#CCDEF7','editorCursor.foreground':mode==='dark'?'#6FB4FF':'#0064D6','editorGutter.background':p.background,'editorIndentGuide.background1':mode==='dark'?'#3A3A3F':'#E2E2E7','editorWidget.background':p.background}
});

export class SourceEditor {
  readonly editor:monaco.editor.IStandaloneCodeEditor;
  private silent=false;
  constructor(host:HTMLElement,onChange:()=>void,onSelection:(line:number)=>void){
    this.editor=monaco.editor.create(host,{value:'',language:'scorestream',theme:'scorestream-light',fontFamily:'"DejaVu Sans Mono", "SFMono-Regular", Consolas, monospace',fontSize:12,lineHeight:22,minimap:{enabled:false},scrollBeyondLastLine:false,wordWrap:'off',automaticLayout:true,renderLineHighlight:'line',overviewRulerLanes:0,hideCursorInOverviewRuler:true,glyphMargin:false,folding:false,lineNumbersMinChars:3,padding:{top:14,bottom:20},tabSize:2,stickyScroll:{enabled:false},accessibilitySupport:'on'});
    this.editor.onDidChangeModelContent(()=>{if(!this.silent)onChange();});
    this.editor.onDidChangeCursorPosition(e=>{if(!this.silent)onSelection(e.position.lineNumber);});
  }
  configure(fontSize:number,wrap:boolean,theme:'light'|'dark'){this.editor.updateOptions({fontSize,wordWrap:wrap?'on':'off'});monaco.editor.setTheme(theme==='dark'?'scorestream-dark':'scorestream-light');}
  dispose(){const model=this.editor.getModel();this.editor.dispose();model?.dispose();}
  get value(){return this.editor.getValue();}
  set value(value:string){this.silent=true;this.editor.setValue(value);this.silent=false;}
  set readOnly(value:boolean){this.editor.updateOptions({readOnly:value});}
  diagnostics(items:Diagnostic[]){monaco.editor.setModelMarkers(this.editor.getModel()!,'rust',items.map(d=>({startLineNumber:Math.max(d.line,1),startColumn:Math.max(d.column,1),endLineNumber:Math.max(d.line,1),endColumn:Math.max(d.column+1,2),message:`${d.code}: ${d.message}`,severity:d.severity==='warning'?monaco.MarkerSeverity.Warning:monaco.MarkerSeverity.Error})));}
  selectLine(line:number){this.silent=true;this.editor.setSelection({startLineNumber:line,startColumn:1,endLineNumber:line,endColumn:1e3});this.editor.revealLineInCenterIfOutsideViewport(line);this.silent=false;}
  focus(){this.editor.focus();}
}
