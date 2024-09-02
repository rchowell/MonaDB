/*
This query extracts the column 'a' from the JSON object.
*/
select json_extract(row, '$.a') from example;
