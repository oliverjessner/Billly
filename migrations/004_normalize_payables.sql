UPDATE invoices
SET
  total_amount = CASE WHEN total_amount LIKE '-%' THEN substr(total_amount, 2) ELSE total_amount END,
  tax_amount = CASE WHEN tax_amount LIKE '-%' THEN substr(tax_amount, 2) ELSE tax_amount END,
  net_amount = CASE WHEN net_amount LIKE '-%' THEN substr(net_amount, 2) ELSE net_amount END
WHERE category = 'payable';
