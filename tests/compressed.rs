//! These are from the "output_styles/compressed/basic" directory in the
//! sass specification.
//! See https://github.com/sass/sass-spec for source material.
//! I add one a test function for one specification at a time and then
//! try to implement that functionality without breaking those already
//! added.
extern crate rsass;
use rsass::{OutputStyle, compile_scss};

#[test]
fn t01_simple_css() {
    check(b"a {\n  \
            color: blue;\n\
            }",
          "a{color:blue}\n")
}

#[test]
fn t02_simple_nesting() {
    check(b"div {\n  img {\n    border: 0px;\n  }\n}",
          "div img{border:0px}\n")
}

#[test]
fn t03_simple_variable() {
    check(b"$color: red;\n\na {\n  color: $color;\n}",
          "a{color:red}\n")
}

#[test]
fn t04_basic_variables() {
    check(b"$color: \"black\";\n$color: red;\n$background: \"blue\";\n\n\
            a {\n  color: $color;\n  background: $background;\n}\n\n\
            $y: before;\n\n\
            $x: 1 2 $y;\n\n\
            foo {\n  a: $x;\n}\n\n\
            $y: after;\n\n\
            foo {\n  a: $x;\n}",
          "a{color:red;background:\"blue\"}foo{a:1 2 before}\
           foo{a:1 2 before}\n")
}

#[test]
fn t05_empty_levels() {
    check(b"div {\n  \
            span {\n    color: red;\n    background: blue;\n  }\n}\n\n\
            div {\n  color: gray;\n\
            empty {\n    \
            span {\n      color: red;\n      background: blue;\n    }\n  \
            }\n}\n\n\
            empty1 {\n  empty2 {\n    \
            div {\n      blah: blah;\n    }\n  }\n}\n\n\
            empty1 {\n  empty2 {\n    div {\n      bloo: blee;\n      \
            empty3 {\n        \
            span {\n          blah: blah;\n          blah: blah;\n        \
            }\n      }\n    }\n  }\n}\n",
          "div span{color:red;background:blue}div{color:gray}\
           div empty span{color:red;background:blue}\
           empty1 empty2 div{blah:blah}empty1 empty2 div{bloo:blee}\
           empty1 empty2 div empty3 span{blah:blah;blah:blah}\n")
}

#[test]
fn t06_nesting_and_comments() {
    // No comments preserved in compressed output!
    check(b"$blah: bloo blee;\n$blip: \"a 'red' and \\\"blue\\\" value\";\n\n\
            /* top level comment -- should be preserved */\n\
            div {\n  /* another comment that should be preserved */\n  \
            color: red;\n  background: blue;\n  $blux: hux; // gone!\n  \
            span {\n    font-weight: bold;\n    \
            a {\n      \
            text-decoration: none; /* where will this comment go? */\n      \
            color: green;\n      \
            /* what about this comment? */ border: 1px $blah red;\n    \
            }\n    \
            /* yet another comment that should be preserved */\n    \
            display: inline-block;\n  }  // gone!\n  \
            /* the next selector should be indented two spaces */\n  \
            empty {\n    \
            not_empty {\n      blah: blah; // gone!\n      bloo: bloo;\n    \
            }\n  }\n  \
            p {\n    padding: 10px 8%;\n    -webkit-box-sizing: $blux;\n  }\n  \
            margin: 10px 5px;\n  h1 {\n    color: $blip;\n  }\n}\n\
            /* last comment, top level again --\n   \
            compare the indentation! */\n\n\
            div {\n\n\
            f: g;\n  \
            empty {\n    span {\n      a: b;\n    }\n  }\n  \
            empty_with_comment {\n    /* hey now */\n    \
            span {\n      c: d;\n    }\n  }\n}",
          "div{color:red;background:blue;margin:10px 5px}\
           div span{font-weight:bold;display:inline-block}\
           div span a{text-decoration:none;color:green;\
           border:1px bloo blee red}\
           div empty not_empty{blah:blah;bloo:bloo}\
           div p{padding:10px 8%;-webkit-box-sizing:hux}\
           div h1{color:\"a 'red' and \\\"blue\\\" value\"}\
           div{f:g}div empty span{a:b}div empty_with_comment span{c:d}\n")
}

#[test]
fn t08_selector_combinators() {
    check(b"a   +   b  >  c {\n  \
            d e {\n    color: blue;\n    background: white;\n  }\n  \
            color: red;\n  background: gray;\n}",
          "a+b>c{color:red;background:gray}\
           a+b>c d e{color:blue;background:white}\n")
}

#[test]
fn t19_full_mixin_craziness() {
    check(b"$x: global-x;\n$y: global-y;\n$z: global-z;\n\n\
            @mixin foo($x, $y) {\n  /* begin foo */\n  \
            margin: $x $y;\n  blip {\n    hey: now;\n  }\n  \
            /* end foo */\n}\n\n\
            @mixin foogoo($x, $y, $z) {\n  margin: $x $y $z;\n}\n\n\
            @mixin hux($y) {\n  /* begin hux */\n  color: $y;\n  \
            @include foo(called-from-hux, $y: $y);\n  /* end hux */\n}\n\n\
            div {\n  @include foo(1, 2);\n  @include foo(1, 3);\n  \
            @include foogoo(1, 2, $z: zee);\n  \
            @include foogoo(1, $y /* blah */ : kwd-y, $z: kwd-z);\n}\n\n\
            div {\n  @include hux($y: $y);\n}\n\n\
            $y: different-global-y;\n\n\
            div {\n  @include hux(calling-hux-again);\n}\n\n\
            @mixin bung() {\n  blah: original-bung;\n}\n\n\
            div {\n  @include bung();\n}\n\n\
            @mixin bung() {\n  blah: redefined-bung;\n}\n\n\
            div {\n  @include bung();\n}\n\n\
            div {\n  /* calls to nullary mixins may omit the empty argument \
            list */\n  @include bung;\n}\n\n\
            div {\n  @include foo($x: kwdarg1, $y: kwdarg2);\n}\n\n\
            @mixin ruleset() {\n  hoo {\n    color: boo;\n  }\n}\n\n\
            @include ruleset();\n\n\
            $da: default argument;\n\n\
            @mixin default_args($x, $y: $da) {\n  blah: $x $y;\n}\n\
            $da: some other default;\n\n\
            div {\n  @include default_args(boogoo);\n}\n\n\
            @mixin original() {\n  value: original;\n}\n\n\
            div {\n  @include original();\n}\n\n\
            @mixin original() {\n  value: no longer original;\n}\n\n\
            div {\n  @include original();\n}\n\n\
            @mixin set-x($x) {\n  $x: changed local x;\n  arg: $x;\n  \
            $y: changed global y !global;\n  blarg: $y;\n}\n\n\
            div {\n  @include set-x(blah);\n  a: $x;\n  b: $y;\n}\n",
          "div{margin:1 2;margin:1 3;margin:1 2 zee;margin:1 kwd-y kwd-z}\
           div blip{hey:now}div blip{hey:now}div{color:global-y;\
           margin:called-from-hux global-y}div blip{hey:now}\
           div{color:calling-hux-again;margin:called-from-hux \
           calling-hux-again}div blip{hey:now}div{blah:original-bung}\
           div{blah:redefined-bung}div{blah:redefined-bung}\
           div{margin:kwdarg1 kwdarg2}div blip{hey:now}hoo{color:boo}\
           div{blah:boogoo some other default}div{value:original}\
           div{value:no longer original}\
           div{arg:changed local x;blarg:changed global y;a:global-x;\
           b:changed global y}\n")
}

#[test]
fn t22_colors_with_alpha() {
    check(b"$x: rgb(0, 255, 255);\n\n\
            div {\n  color: rgb(255, $blue: 0, $green: 255);\n  \
            background: rgb(123, 45, 6);\n  flah: rgba(0, 0, 0, 1) + #111;\n  \
            grah: rgba(#f0e, $alpha: .5);\n  blah: rgba(1,2,3,.6);\n  \n  \
            floo: $x;\n  bloo: rgba($x, 0.7);\n  groo: $x;\n\n\
            $x: rgb(123, 45, 6);\n  \n  \
            hoo: red($x);\n  moo: green($x);\n  poo: blue($x);\n  \n  \
            goo: mix(rgba(255, 0, 0, 0.5), #00f);\n  \n  \
            boo: invert(#123456);\n}\n",
          "div{color:#ff0;background:#7b2d06;flah:#111;\
           grah:rgba(255,0,238,0.5);blah:rgba(1,2,3,0.6);floo:cyan;\
           bloo:rgba(0,255,255,0.7);groo:cyan;hoo:123;moo:45;poo:6;\
           goo:rgba(64,0,191,0.75);boo:#edcba9}
")
}

fn check(input: &[u8], expected: &str) {
    assert_eq!(compile_scss(input, OutputStyle::Compressed).and_then(|s| {
                   String::from_utf8(s)
                       .map_err(|e| format!("Non-utf8 output: {}", e))
               }),
               Ok(expected.into()));
}