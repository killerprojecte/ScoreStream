use std::{fs,process::Command};
#[test]
fn final_ending_policy_is_explicit_on_both_conversion_directions(){
 let dir=std::env::temp_dir().join(format!("scorestream-final-ending-{}",std::process::id()));fs::create_dir_all(&dir).unwrap();
 let xml=dir.join("input.musicxml");let native=dir.join("output.sfs");
 let source=include_str!("../../scorestream-formats/tests/fixtures/navigation-closed.musicxml").replace("<barline location=\"right\"><ending number=\"2\" type=\"discontinue\"/></barline>","");fs::write(&xml,&source).unwrap();
 let run=|from:&str,to:&str,path:&std::path::Path,args:&[&str]|Command::new(env!("CARGO_BIN_EXE_scorestream")).args(["convert",from,to]).arg(path).args(args).output().unwrap();
 let policy=["--unclosed-final-ending","continue-to-movement-end"];
 let strict=run("musicxml","sfs",&xml,&[]);assert!(!strict.status.success());assert!(strict.stdout.is_empty());
 let accepted=run("musicxml","sfs",&xml,&policy);assert!(accepted.status.success(),"{}",String::from_utf8_lossy(&accepted.stderr));assert!(String::from_utf8_lossy(&accepted.stderr).contains("navigation.unclosed_final_ending_continues"));fs::write(&native,&accepted.stdout).unwrap();
 assert!(!run("sfs","musicxml",&native,&[]).status.success());
 let restored=run("sfs","musicxml",&native,&policy);assert!(restored.status.success(),"{}",String::from_utf8_lossy(&restored.stderr));let output=String::from_utf8(restored.stdout).unwrap();assert!(!output.contains("type=\"discontinue\""));assert_eq!(output.matches("<note ").count(),4);
 for args in [vec!["--unclosed-final-ending","guess"],vec![policy[0],policy[1],policy[0],policy[1]]]{let invalid=run("musicxml","sfs",&xml,&args);assert_eq!(invalid.status.code(),Some(2));assert!(invalid.stdout.is_empty());}
 let invalid=run("musicxml","musicxml",&xml,&policy);assert_eq!(invalid.status.code(),Some(2));
 fs::remove_dir_all(dir).unwrap();
}
