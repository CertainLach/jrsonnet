{
	lstripChars_singleChar: std.lstripChars("aaabcdef", "a"),
	lstripChars_multipleChars: std.lstripChars("klmn", "kql"),
	lstripChars_array: std.lstripChars("forward", [1, "f", [], "o", "d", "for"]),

	rstripChars_singleChar: std.rstripChars("nice_boy", "y"),
	rstripChars_multipleChars: std.rstripChars("amoguass", "sa"),
	rstripChars_array: std.rstripChars("cool just cool", ["o", "l", 12.2323443]),

	stripChars_singleCharL: std.stripChars("feefoofaa", "f"),
	stripChars_singleCharR: std.stripChars("lolkekw", "w"),
	stripChars_singleChar: std.stripChars("joper jej", "j"),

	stripChars_multipleCharsL: std.stripChars("abcdefg", "cab"),
	stripChars_multipleCharsR: std.stripChars("still breathing", "gthin"),
	stripChars_multipleChars: std.stripChars("sus sus sus", "us"),

	stripChars_arrayL: std.stripChars("chel medvedo svin", ["c", 3204990, {"svin": {}}, "vi"]),
	stripChars_arrayR: std.stripChars("lach-vs-miri", ["r", "i", "craft", "is", "mine"]),
	stripChars_array: std.stripChars("UwU Lel Stosh", ["h", "U", "s", {}, [], null, "w", [1, 2, 3]]),
}
