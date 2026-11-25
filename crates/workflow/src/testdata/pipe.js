export const meta = { name: 'pipe', description: 'two stages per item' }
return await pipeline(args, (item) => agent('draft ' + item), (draft, item, i) => agent('check ' + draft + ' #' + i))