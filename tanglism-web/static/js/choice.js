export const choice = {
    data,
    draw_table
};

const _data = [];
const table_header = {
  names: ["code", "display_name", "msci", "hs300", "choice"],
  display_names: ["股票代码", "股票名称", "MSCI", "沪深300", "买卖点"],
  sort_ascending: [false, false, false, false, false]
};

function data(input) {
    if (input) {
      while(_data.length > 0) { _data.pop(); }
      for (var i = 0; i < input.length; i++) {
        _data.push(input[i]);
      }
      return;
    }
    return _data;
};

function draw_table() {
    // 刷新表格
    if (!d3.select("#choice_table").empty()) {
      d3.select("#choice_table").remove();
    }
    var table = d3.select("#choice_container").append("table")
      .attr("id", "choice_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(table_header.display_names)
      .enter()
      .append("th")
      .text(function(d) {return d;})
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("background-color", "lightgray")
      .style("font-weight", "bold")
      .on("click", function(d, i){
          // 重排序
          const name = table_header.names[i];
          var cmp = function(a,b) {
            if (a[name] === undefined) {
              return -1;
            }
            if (b[name] === undefined) {
              return 1;
            }
            if (a[name] < b[name]) {
              return -1;
            }
            if (a[name] > b[name]) {
              return 1;
            }
            return 0;
          };
          // 正序和逆序交错
          if (!table_header.sort_ascending[i]) {
            var tmp = cmp;
            cmp = function(a, b) {
              return tmp(b, a);
            }
          }
          _data.sort(cmp);
          draw_table();
          table_header.sort_ascending[i] = !table_header.sort_ascending[i];
      });
    
    // 内容
    table.append("tbody")
      .selectAll("tr")
      .data(_data)
      .enter()
      .append("tr")
      .selectAll("td")
      .data(function(d) {
        var msci = "N";
        if (d.msci) { msci = "Y"; }
        var hs300 = "N";
        if (d.hs300) { hs300 = "Y"; }
        var choice = "";
        if (d.choice === "BuyOne") {
            choice = "一买";
        } else if (d.choice === "BuyTwo") {
            choice = "二买";
        } else if (d.choice === "BuyThree") {
            choice = "三买";
        }
        return [
          d.code, 
          d.display_name, 
          msci,
          hs300,
          choice
        ];
      })
      .enter()
      .append("td")
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("font-size", "12px")
      .text(function(d) {return d;})
      .on("mouseover", function(){
        d3.select(this).style("background-color", "powderblue");
      })
      .on("mouseout", function(){
        d3.select(this).style("background-color", "white");
      });
};
