local str = 'ab::cd::ef';
{
	split: std.split(str, '::'),
	splitlimit: std.splitLimit(str, '::', 1),
	splitlimitRNoLimit: std.splitLimit(str, '::', -1),
	splitlimitR: std.splitLimitR(str, '::', 1),
}
