declare i32 @getint() ; extern

declare void @putint(i32) ; extern

define dso_local i32 @main() {
0:
    ; ==== Phi Section End ====
    %1 = alloca i32 , align 4
    %2 = alloca i32 , align 4
    %3 = alloca i32 , align 4
    %4 = call i32 @getint()
    store i32 %4, ptr %1, align 4
    %5 = call i32 @getint()
    store i32 %5, ptr %2, align 4
    %6 = load i32, ptr %1, align 4
    %7 = load i32, ptr %2, align 4
    %8 = icmp slt i32 %6, %7
    br i1 %8, label %9, label %13
9:
    ; ==== Phi Section End ====
    %10 = load i32, ptr %1, align 4
    store i32 %10, ptr %3, align 4
    %11 = load i32, ptr %2, align 4
    store i32 %11, ptr %1, align 4
    %12 = load i32, ptr %3, align 4
    store i32 %12, ptr %2, align 4
    br label %13
13:
    ; ==== Phi Section End ====
    %14 = load i32, ptr %1, align 4
    call void @putint(i32 %14)
    %15 = load i32, ptr %2, align 4
    call void @putint(i32 %15)
    ret i32 0
}
