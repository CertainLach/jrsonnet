// Outer local captured and used inside a nested function definition
local x = 10;
local f = function(n) n + x;
f(1)
