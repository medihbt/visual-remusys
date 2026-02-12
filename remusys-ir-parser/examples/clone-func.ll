@str_red    = internal constant [3 x i8] c"red", align 8
@str_green  = internal constant [5 x i8] c"green", align 8
@str_blue   = internal constant [4 x i8] c"blue", align 8

define dso_local {ptr, i64} @color_get_name(i8 %color) {
entry:
    switch i8 %color, label %default [
        i8 0, label %case_red
        i8 1, label %case_green
        i8 2, label %case_blue
    ]
case_red:
    ; ret {ptr, i64} {ptr @str_red, i64 3}
    br label %finish
case_green:
    ; ret {ptr, i64} {ptr @str_green, i64 5}
    br label %finish
case_blue:
    ; ret {ptr, i64} {ptr @str_blue, i64 4}
    br label %finish
default:
    unreachable
finish:
    %retval = phi {ptr, i64}
        [{ptr @str_red, i64 3}, %case_red],
        [{ptr @str_green, i64 5}, %case_green],
        [{ptr @str_blue, i64 4}, %case_blue]
    ret {ptr, i64} %retval
}

define dso_local i64 @fibonacci(i8 %n) {
    switch i8 %n, label %recurse [
        i8 0, label %direct_ret
        i8 1, label %direct_ret
    ]
direct_ret:
    %retval = zext i8 %n to i64
    ret i64 %retval
recurse:
    %sub1 = sub nuw i8 %n, 1
    %sub2 = sub nuw i8 %n, 2
    %res1 = call i64 @fibonacci(i8 %sub1)
    %res2 = call i64 @fibonacci(i8 %sub2)
    %addret = add nuw i64 %res1, %res2
    ret i64 %addret
}
