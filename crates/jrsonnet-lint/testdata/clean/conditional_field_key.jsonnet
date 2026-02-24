// Local used in a conditional dynamic field key [if cond then 'key']
local env = 'prod';
{
  [if env == 'prod' then 'prod_key']: 1,
}
